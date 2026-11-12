//! The engine's PostgreSQL store (withhuman-core #118), read the way the Neo4j-era API answered,
//! as withhuman-core's tools/graph-viewer/server.py does: a node's properties are its row without
//! the columns that stand for edges, and the edges are rebuilt from those columns.
//! - CONTAINS is `session_id`; MEMBER is `subgraph`.
//! - RESULT_OF is a Call and a result sharing `tool_use_id`.
//! - The structural NEXT leading to a node is its `next_seq` and `next_via`.
//! - NEXT {kind: 'cause'} is `cause_edges`; MENTIONS_TOKEN is `token_mentions`.
//!
//! Each query's rows come back as one JSON array (`json_agg`), so the column types need no
//! mapping and the answers are the same `serde_json` values as from Neo4j or the demo. Text the
//! store keeps as JSON (a payload, a trace) is cut with regular expressions, not cast to `jsonb`,
//! so one malformed document never fails a whole query.

use std::str::FromStr;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use serde_json::{Map, Value, json};
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, Config};

use crate::graph::{ApiError, Rows, call_row, check_idle, clip, preview, summary};
use crate::settings::Settings;

mod tls;

/// A trace whose `irreversible` is at least one half, as PostgreSQL's regular expressions read
/// it: the same match as [`crate::graph::IRREVERSIBLE_TRACE`] without the anchors `~` needs not.
pub const IRREVERSIBLE: &str = r#""irreversible":(0\.[5-9][0-9]*|1(\.0*)?)[,}]"#;

/// The columns that stand for edges rather than a node's own properties.
const EDGE_COLUMNS: [&str; 3] = ["kind", "next_seq", "next_via"];

/// The text of a string field of the JSON kept in `column`, still escaped as JSON
/// ([`unescape`] reads it).
fn field(column: &str, key: &str) -> String {
    format!(r#"substring({column} from '"{key}":"((?:[^"\\]|\\.)*)"')"#)
}

/// Whether a Call's tool ran: a result sharing its `tool_use_id` came back.
fn ran(call: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM nodes r WHERE r.session_id = {call}.session_id AND r.tool_use_id = {call}.tool_use_id \
         AND r.kind IN ('Segment', 'Error'))"
    )
}

static SESSIONS: LazyLock<String> = LazyLock::new(|| {
    r#"
SELECT s.session_id AS id, s.version, s.created_at,
       c.nodes, c.calls, c.denied, c.reviewed, c.allowed, c.failed, c.undecided, c.risky, c.from_result, c.last_at,
       (SELECT payload_json FROM nodes WHERE session_id = s.session_id AND kind = 'Request'
        ORDER BY "order" LIMIT 1) AS first_request,
       (SELECT intent FROM nodes WHERE session_id = s.session_id AND kind = 'Intent'
        ORDER BY "order" DESC LIMIT 1) AS intent,
       (SELECT cwd FROM nodes WHERE session_id = s.session_id AND cwd IS NOT NULL
        ORDER BY "order" LIMIT 1) AS cwd,
       (SELECT client_id FROM nodes WHERE session_id = s.session_id AND client_id IS NOT NULL
        ORDER BY "order" LIMIT 1) AS client,
       (SELECT decision_mode FROM nodes WHERE session_id = s.session_id AND kind = 'Call'
        AND decision_mode IS NOT NULL ORDER BY decided_at DESC NULLS LAST LIMIT 1) AS mode
FROM (SELECT * FROM sessions WHERE created_at IS NOT NULL ORDER BY created_at DESC LIMIT 5000) s
CROSS JOIN LATERAL (
  SELECT count(*) AS nodes,
         count(*) FILTER (WHERE kind = 'Call') AS calls,
         count(*) FILTER (WHERE kind = 'Call' AND decision = 'DENY') AS denied,
         count(*) FILTER (WHERE kind = 'Call' AND decision = 'REVIEW') AS reviewed,
         count(*) FILTER (WHERE kind = 'Call' AND decision = 'ALLOW') AS allowed,
         count(*) FILTER (WHERE kind = 'Call' AND trace_error IS NOT NULL) AS failed,
         count(*) FILTER (WHERE kind = 'Call' AND decision IS NULL) AS undecided,
         count(*) FILTER (WHERE kind = 'Call' AND decision = 'ALLOW' AND trace_json ~ $1) AS risky,
         count(*) FILTER (WHERE kind = 'Call' AND trace_json LIKE '%"link":"segment"%') AS from_result,
         max(observed_at) AS last_at
  FROM nodes WHERE session_id = s.session_id
) c
ORDER BY s.created_at DESC
"#
    .to_owned()
});

/// Every Call cut down to what the security team's charts count, as `graph::CALL_FACTS`.
static CALL_FACTS: LazyLock<String> = LazyLock::new(|| {
    format!(
        r#"
SELECT c.session_id AS session, c.seq, c.client_id AS client, c.cwd,
       left(c.decision_reason, 300) AS reason,
       (SELECT {prompt} FROM nodes r WHERE r.session_id = c.session_id AND r.kind = 'Request'
        AND r."order" < c."order" ORDER BY r."order" DESC LIMIT 1) AS prompt,
       coalesce({command}, {file_path}, {url}, {pattern}) AS target,
       {ran} AS ran,
       (SELECT min(r.observed_at) FROM nodes r WHERE r.session_id = c.session_id
        AND r.tool_use_id = c.tool_use_id AND r.kind IN ('Segment', 'Error')) AS ran_at,
       coalesce(c.decided_at, c.observed_at) AS at,
       c.decision, c.decision_mode AS mode, c.decision_rule AS rule,
       {tool} AS tool,
       substring(c.trace_json from '"link":"([^"]*)"') AS link,
       substring(c.trace_json from '"worst":"([^"]*)"') AS worst,
       coalesce(c.trace_json ~ $1, false) AS irreversible
FROM nodes c WHERE c.kind = 'Call'
ORDER BY at DESC NULLS LAST LIMIT 50000
"#,
        prompt = field("r.payload_json", "prompt"),
        command = field("c.payload_json", "command"),
        file_path = field("c.payload_json", "file_path"),
        url = field("c.payload_json", "url"),
        pattern = field("c.payload_json", "pattern"),
        tool = field("c.payload_json", "tool_name"),
        ran = ran("c"),
    )
});

/// Every mention of a sensitive value, as `graph::SECRETS`.
static SECRETS: LazyLock<String> = LazyLock::new(|| {
    format!(
        r#"
SELECT m.session_id AS session, n.seq, n.kind AS node, m.slot,
       t.key, t.key_kind AS kind, t.category, coalesce(t.decoded, t.key) AS text,
       n.decision, n.client_id AS client, coalesce(n.decided_at, n.observed_at) AS at,
       {tool} AS tool,
       coalesce({command}, {url}, {file_path}) AS target,
       (n.kind = 'Call' AND {ran}) AS ran
FROM token_mentions m
JOIN tokens t USING (session_id, key_kind, key, slot)
JOIN nodes n ON n.session_id = m.session_id AND n.seq = m.node_seq
WHERE t.category IN ('credential', 'pii', 'financial', 'device')
ORDER BY at DESC NULLS LAST LIMIT 20000
"#,
        tool = field("n.payload_json", "tool_name"),
        command = field("n.payload_json", "command"),
        url = field("n.payload_json", "url"),
        file_path = field("n.payload_json", "file_path"),
        ran = ran("n"),
    )
});

const NODES: &str = r#"SELECT * FROM nodes WHERE session_id = $1 ORDER BY "order""#;

const SUBGRAPHS: &str = r#"
SELECT g.*, COALESCE((SELECT array_agg(seq ORDER BY "order") FROM nodes
                      WHERE session_id = g.session_id AND subgraph = g.number), '{}') AS members
FROM subgraphs g WHERE g.session_id = $1 ORDER BY g.number
"#;

const NEXT_EDGES: &str =
    "SELECT next_seq AS source, seq AS target, next_via AS via FROM nodes WHERE session_id = $1 AND next_seq IS NOT NULL";

const RESULT_EDGES: &str = r#"
SELECT r.seq AS source, c.seq AS target FROM nodes r
JOIN nodes c ON c.session_id = r.session_id AND c.tool_use_id = r.tool_use_id AND c.kind = 'Call'
WHERE r.session_id = $1 AND r.kind IN ('Segment', 'Error')
"#;

const CAUSE_EDGES: &str = "SELECT call_seq, target_seq, target_index, probability, rank, parent FROM cause_edges \
                           WHERE session_id = $1";

const TOKENS: &str = "SELECT node_seq AS seq, key, key_kind AS kind, slot, category, coalesce(decoded, key) AS text \
                      FROM tokens WHERE session_id = $1 ORDER BY node_seq";

const NODE: &str = "SELECT * FROM nodes WHERE session_id = $1 AND seq = $2";

const PARAGRAPHS: &str = r#"SELECT "index", hidden, hidden_rules, text, source, instructs FROM paragraphs
WHERE session_id = $1 AND seq = $2 ORDER BY "index""#;

const NODE_TOKENS: &str = "SELECT t.key_kind AS kind, t.category, m.slot, t.decoded FROM token_mentions m \
                           JOIN tokens t USING (session_id, key_kind, key, slot) \
                           WHERE m.session_id = $1 AND m.node_seq = $2";

static CALLS: LazyLock<String> = LazyLock::new(|| {
    format!(
        r#"SELECT c.*, {ran} AS ran FROM nodes c WHERE c.session_id = ANY($1) AND c.kind = 'Call' ORDER BY c."order""#,
        ran = ran("c")
    )
});

const LAST_ACTIVE: &str = r#"
SELECT count(*) AS rows, max(observed_at) AS last_at FROM (
  SELECT observed_at FROM nodes WHERE session_id = $1 AND observed_at IS NOT NULL
  UNION ALL
  SELECT observed_at FROM event_commits WHERE session_id = $1 AND observed_at IS NOT NULL
) a
"#;

/// Every table holding a session's rows: the first ones are what Neo4j kept as its nodes and
/// count as deleted, the rest were its edges. Files are shared and stay.
const SESSION_TABLES: [&str; 8] = [
    "sessions",
    "event_commits",
    "nodes",
    "subgraphs",
    "paragraphs",
    "tokens",
    "pending_paragraph_enrichments",
    "pending_request_continuations",
];
const EDGE_TABLES: [&str; 3] = ["token_mentions", "file_refs", "cause_edges"];

/// The connection kept open between reads, made again when the settings change or it closes.
#[derive(Default)]
pub struct Pool {
    slot: Mutex<Option<(String, Arc<Client>)>>,
}

impl Pool {
    async fn client(&self, s: &Settings) -> Result<Arc<Client>, ApiError> {
        let key = format!("{}\u{1}{}\u{1}{}\u{1}{}", s.neo4j_url, s.database, s.user, s.password);
        if let Some((k, c)) = self.slot.lock().expect("pool lock").clone()
            && k == key
            && !c.is_closed()
        {
            return Ok(c);
        }
        let client = Arc::new(connect(config(s, true)?, &s.neo4j_url).await?);
        *self.slot.lock().expect("pool lock") = Some((key, client.clone()));
        Ok(client)
    }

    fn forget(&self) {
        *self.slot.lock().expect("pool lock") = None;
    }
}

/// Whether the settings point at PostgreSQL rather than Neo4j: the address is a `postgres://` URL.
pub fn is_postgres(s: &Settings) -> bool {
    let url = s.neo4j_url.trim();
    url.starts_with("postgres://") || url.starts_with("postgresql://")
}

/// The connection the settings name: the address, then the user, password and database from the
/// form unless the address already says them, and its encryption ([`tls`]). Reads run read-only.
fn config(s: &Settings, read_only: bool) -> Result<(Config, tls::Connector), ApiError> {
    let address = tls::Address::read(&s.neo4j_url);
    let mut c = Config::from_str(&address.url)
        .map_err(|e| ApiError::new(400, format!("데이터베이스 주소를 읽지 못했습니다: {e}")))?;
    let (mode, check) = address.plan(c.get_hosts()).map_err(|e| ApiError::new(400, e))?;
    c.ssl_mode(mode);
    if c.get_user().is_none() && !s.user.is_empty() {
        c.user(&s.user);
    }
    if c.get_password().is_none() && !s.password.is_empty() {
        c.password(&s.password);
    }
    if c.get_dbname().is_none() && !s.database.is_empty() {
        c.dbname(&s.database);
    }
    c.connect_timeout(Duration::from_secs(5));
    c.application_name("withhuman-viewer");
    c.options(if read_only {
        "-c statement_timeout=30000 -c default_transaction_read_only=on"
    } else {
        "-c statement_timeout=30000"
    });
    let tls = address.connector(check).map_err(|e| ApiError::new(400, e))?;
    Ok((c, tls))
}

async fn connect((config, tls): (Config, tls::Connector), url: &str) -> Result<Client, ApiError> {
    let (client, connection) = config.connect(tls).await.map_err(|e| error(&e, url))?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

/// What a failure means to the person at the settings. The address is named, the password never.
fn error(e: &tokio_postgres::Error, url: &str) -> ApiError {
    let host = url.split('@').next_back().unwrap_or(url);
    let host = host.split('?').next().unwrap_or(host);
    if let Some(trouble) = tls::trouble(e, host) {
        return ApiError::new(502, trouble);
    }
    if let Some(db) = e.as_db_error() {
        return match db.code().code() {
            "28P01" | "28000" => ApiError::new(
                401,
                "PostgreSQL이 사용자 이름이나 비밀번호를 받지 않았습니다. 엔진의 뷰어 계정(viewer-role.sql)과 \
                 비밀번호를 연결 설정에 넣어 주세요.",
            ),
            "42501" => ApiError::new(
                403,
                format!(
                    "이 계정에는 읽을 권한이 없는 표가 있습니다 ({}). 엔진에 viewer-role.sql을 적용했는지 확인해 주세요.",
                    db.message()
                ),
            ),
            "3D000" => ApiError::new(404, format!("데이터베이스를 찾지 못했습니다 ({}).", db.message())),
            _ => ApiError::new(500, db.message().to_owned()),
        };
    }
    ApiError::new(
        502,
        format!(
            "{host}에서 PostgreSQL이 응답하지 않습니다. WithHuman 엔진이 실행 중인지, 포트가 열려 있는지 확인해 주세요. ({e})"
        ),
    )
}

/// A row as Neo4j's `properties()`: its columns that hold a value, minus `drop`.
fn properties(row: Map<String, Value>, drop: &[&str]) -> Map<String, Value> {
    row.into_iter().filter(|(k, v)| !v.is_null() && !drop.contains(&k.as_str())).collect()
}

/// A field cut out of stored JSON by [`field`], read back as the text it stands for.
fn unescape(row: &mut Map<String, Value>, key: &str, limit: usize) {
    if let Some(Value::String(raw)) = row.get(key) {
        let text = serde_json::from_str::<String>(&format!("\"{raw}\"")).unwrap_or_else(|_| raw.clone());
        row.insert(key.to_owned(), Value::String(clip(&text, limit)));
    }
}

pub struct Postgres<'a> {
    pub settings: &'a Settings,
    pub pool: &'a Pool,
}

impl Postgres<'_> {
    /// The rows of `sql`, each a JSON object of its columns; a closed connection is opened again
    /// once.
    async fn rows(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Rows, ApiError> {
        let wrapped = format!("SELECT coalesce(json_agg(q), '[]'::json)::text FROM ({sql}) q");
        let mut tries = 0;
        loop {
            let client = self.pool.client(self.settings).await?;
            match client.query_one(&wrapped, params).await {
                Ok(row) => {
                    let text: String = row.get(0);
                    return serde_json::from_str(&text)
                        .map_err(|e| ApiError::new(500, format!("PostgreSQL의 답을 읽지 못했습니다: {e}")));
                }
                Err(e) if e.is_closed() && tries == 0 => {
                    self.pool.forget();
                    tries += 1;
                }
                Err(e) => return Err(error(&e, &self.settings.neo4j_url)),
            }
        }
    }

    /// How many sessions the store holds: the settings' test.
    pub async fn count_sessions(&self) -> Result<i64, ApiError> {
        let rows = self.rows("SELECT count(*) AS sessions FROM sessions", &[]).await?;
        Ok(rows.first().and_then(|r| r.get("sessions")).and_then(Value::as_i64).unwrap_or(0))
    }

    /// The sessions, newest first, each with its first prompt's text rather than the Request's
    /// payload.
    pub async fn sessions(&self) -> Result<Value, ApiError> {
        let mut rows = self.rows(&SESSIONS, &[&IRREVERSIBLE]).await?;
        rows.sort_by_key(|r| std::cmp::Reverse(r.get("created_at").and_then(Value::as_i64)));
        for row in &mut rows {
            let request = row
                .remove("first_request")
                .and_then(|v| v.as_str().and_then(|s| serde_json::from_str::<Value>(s).ok()))
                .unwrap_or(Value::Null);
            let prompt = request.get("prompt").and_then(Value::as_str).map(|p| clip(p, 300));
            row.insert("prompt".into(), prompt.map_or(Value::Null, Value::String));
        }
        Ok(Value::from(rows.into_iter().map(Value::Object).collect::<Vec<_>>()))
    }

    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn call_facts(&self) -> Result<Value, ApiError> {
        let mut rows = self.rows(&CALL_FACTS, &[&IRREVERSIBLE]).await?;
        for row in &mut rows {
            unescape(row, "prompt", 200);
            unescape(row, "target", 300);
            unescape(row, "tool", 200);
        }
        Ok(Value::from(rows.into_iter().map(Value::Object).collect::<Vec<_>>()))
    }

    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn secrets(&self) -> Result<Value, ApiError> {
        let mut rows = self.rows(&SECRETS, &[]).await?;
        for row in &mut rows {
            unescape(row, "target", 300);
            unescape(row, "tool", 200);
        }
        Ok(Value::from(rows.into_iter().map(Value::Object).collect::<Vec<_>>()))
    }

    /// The engine's agent inventory, or null for an engine that keeps none yet (no such tables).
    /// A key's hash is never read.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn inventory(&self) -> Result<Value, ApiError> {
        let found = self.rows("SELECT to_regclass('public.clients') IS NOT NULL AS ok", &[]).await?;
        if !found.first().and_then(|r| r.get("ok")).and_then(Value::as_bool).unwrap_or(false) {
            return Ok(Value::Null);
        }
        let all = |rows: Rows| Value::from(rows.into_iter().map(Value::Object).collect::<Vec<_>>());
        Ok(json!({
            "clients": all(self.rows("SELECT * FROM clients", &[]).await?),
            "projects": all(self.rows("SELECT * FROM client_projects", &[]).await?),
            "users": all(self.rows("SELECT * FROM users", &[]).await?),
            "keys": all(self.rows(
                "SELECT key_id, user_email, created_at, created_by, note, last_used_at, revoked_at, revoked_by, \
                 revoke_reason, first_denied_at, last_denied_at, denied_count FROM api_keys", &[]).await?),
            "events": all(self.rows("SELECT * FROM key_events ORDER BY at DESC LIMIT 5000", &[]).await?),
            "sessions": all(self.rows(
                "SELECT session_id, client_id, user_email, start_source, permission_mode, model, created_at, \
                 last_seen_at, ended_at, end_reason FROM sessions WHERE created_at IS NOT NULL \
                 ORDER BY created_at DESC LIMIT 5000", &[]).await?),
        }))
    }

    pub async fn graph(&self, session: &str) -> Result<Value, ApiError> {
        let mut nodes = self.rows(NODES, &[&session]).await?;
        nodes.sort_by_key(|r| r.get("order").and_then(Value::as_i64));
        let nodes: Vec<Value> = nodes
            .into_iter()
            .map(|row| {
                let kind = row.get("kind").cloned().unwrap_or(Value::Null);
                let props = properties(row, &EDGE_COLUMNS);
                json!({"kind": kind, "summary": summary(&props), "props": preview(props)})
            })
            .collect();
        let subgraphs: Vec<Value> = self
            .rows(SUBGRAPHS, &[&session])
            .await?
            .into_iter()
            .map(|mut row| {
                let members = row.remove("members").unwrap_or(json!([]));
                json!({"props": properties(row, &[]), "members": members})
            })
            .collect();
        let mut edges: Vec<Value> = Vec::new();
        for r in self.rows(NEXT_EDGES, &[&session]).await? {
            let (source, target) = (r["source"].clone(), r["target"].clone());
            edges.push(json!({
                "source": source, "target": target, "type": "NEXT", "paragraph": null,
                "props": {"via": r["via"], "edge_id": format!("{session}:{source}:{target}"), "probability": 1.0},
            }));
        }
        for r in self.rows(RESULT_EDGES, &[&session]).await? {
            edges.push(json!({"source": r["source"], "target": r["target"], "type": "RESULT_OF", "props": {}, "paragraph": null}));
        }
        for r in self.rows(CAUSE_EDGES, &[&session]).await? {
            let index = r["target_index"].as_i64().unwrap_or(-1);
            let props = properties(
                json!({"kind": "cause", "call_seq": r["call_seq"], "probability": r["probability"],
                       "rank": r["rank"], "parent": r["parent"]})
                .as_object()
                .cloned()
                .unwrap_or_default(),
                &[],
            );
            edges.push(json!({
                "source": r["call_seq"], "target": r["target_seq"], "type": "NEXT", "props": props,
                "paragraph": if index >= 0 { json!(index) } else { Value::Null },
            }));
        }
        let tokens = self.rows(TOKENS, &[&session]).await?;
        Ok(json!({"nodes": nodes, "subgraphs": subgraphs, "edges": edges, "tokens": tokens}))
    }

    pub async fn node(&self, session: &str, seq: i64) -> Result<Value, ApiError> {
        let Some(row) = self.rows(NODE, &[&session, &seq]).await?.into_iter().next() else {
            return Ok(Value::Null);
        };
        let kind = row.get("kind").cloned().unwrap_or(Value::Null);
        let paragraphs = self.rows(PARAGRAPHS, &[&session, &seq]).await?;
        let tokens = self.rows(NODE_TOKENS, &[&session, &seq]).await?;
        Ok(json!({"kind": kind, "props": properties(row, &EDGE_COLUMNS), "paragraphs": paragraphs, "tokens": tokens}))
    }

    /// Each session's Calls: tool, verdict and deviation, without payload or trace.
    pub async fn calls(&self, sessions: &[String]) -> Result<Value, ApiError> {
        let mut found: Map<String, Value> = sessions.iter().map(|s| (s.clone(), Value::Array(Vec::new()))).collect();
        let list: Vec<&str> = sessions.iter().map(String::as_str).collect();
        for row in self.rows(&CALLS, &[&list]).await? {
            let ran = row.get("ran").cloned().unwrap_or(Value::Bool(false));
            let props = properties(row, &["kind", "next_seq", "next_via", "ran"]);
            let session = props.get("session_id").and_then(Value::as_str).unwrap_or_default().to_owned();
            if let Some(Value::Array(calls)) = found.get_mut(&session) {
                calls.push(call_row(&props, ran));
            }
        }
        Ok(Value::Object(found))
    }

    /// Deletes a session no longer active, in one transaction on a connection of its own.
    pub async fn delete_session(&self, session: &str) -> Result<Value, ApiError> {
        let url = &self.settings.neo4j_url;
        let mut client = connect(config(self.settings, false)?, url).await?;
        let tx = client.transaction().await.map_err(|e| error(&e, url))?;
        // Deleting the session row first takes the lock the engine holds while it commits an
        // event, so no event lands between the activity check and the delete.
        let mut deleted = tx
            .execute("DELETE FROM sessions WHERE session_id = $1", &[&session])
            .await
            .map_err(|e| error(&e, url))? as i64;
        let found = tx.query_one(LAST_ACTIVE, &[&session]).await.map_err(|e| error(&e, url))?;
        let rows: i64 = found.get("rows");
        let last_at: Option<i64> = found.get("last_at");
        if deleted == 0 && rows == 0 {
            return Err(ApiError::new(404, "세션을 찾지 못했습니다."));
        }
        check_idle(last_at)?;
        for table in SESSION_TABLES[1..].iter().chain(EDGE_TABLES.iter()) {
            let removed = tx
                .execute(&format!("DELETE FROM {table} WHERE session_id = $1"), &[&session])
                .await
                .map_err(|e| error(&e, url))?;
            if SESSION_TABLES.contains(table) {
                deleted += removed as i64;
            }
        }
        tx.commit().await.map_err(|e| error(&e, url))?;
        Ok(json!({"deleted": deleted}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_choose_postgres_by_the_address() {
        let mut s = Settings::default();
        s.neo4j_url = "postgres://127.0.0.1:55432/withhuman".into();
        assert!(is_postgres(&s));
        s.neo4j_url = "http://127.0.0.1:7474".into();
        assert!(!is_postgres(&s));
    }

    #[test]
    fn the_form_fills_what_the_address_leaves_out() {
        let mut s = Settings::default();
        s.neo4j_url = "postgres://127.0.0.1:55432".into();
        s.database = "withhuman".into();
        s.user = "withhuman_viewer".into();
        s.password = "secret".into();
        let (c, _) = config(&s, true).unwrap();
        assert_eq!(c.get_dbname(), Some("withhuman"));
        assert_eq!(c.get_user(), Some("withhuman_viewer"));
        s.neo4j_url = "postgres://reader@host/graph".into();
        let (c, _) = config(&s, true).unwrap();
        assert_eq!((c.get_dbname(), c.get_user()), (Some("graph"), Some("reader")));
    }

    #[test]
    fn a_cut_field_reads_back_as_its_text() {
        let mut row = Map::new();
        row.insert("target".into(), json!(r#"echo \"hi\"\nls"#));
        unescape(&mut row, "target", 300);
        assert_eq!(row["target"], json!("echo \"hi\"\nls"));
    }

    /// The connection is encrypted, or not, as the address asks: VIEWER_PG_URL is the address,
    /// VIEWER_PG_EXPECT `encrypted`, `plain` or a piece of the error the settings would show.
    #[test]
    #[ignore = "needs a PostgreSQL"]
    fn the_connection_is_as_the_address_asks() {
        let url = std::env::var("VIEWER_PG_URL").expect("VIEWER_PG_URL");
        let expect = std::env::var("VIEWER_PG_EXPECT").expect("VIEWER_PG_EXPECT");
        let mut s = Settings::default();
        s.neo4j_url = url.clone();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let got = rt.block_on(async {
            let client = match async { connect(config(&s, true)?, &url).await }.await {
                Ok(client) => client,
                Err(e) => return e.error,
            };
            let row = client
                .query_one("SELECT coalesce((SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()), false)", &[])
                .await
                .unwrap();
            if row.get::<_, bool>(0) { "encrypted".to_owned() } else { "plain".to_owned() }
        });
        assert!(got.contains(&expect), "expected {expect}, got {got}");
    }

    /// The store answers as the demo does for the same graph. Load a PostgreSQL holding the
    /// engine's schema with the fixture, then run with its address:
    ///
    /// ```sh
    /// python3 scripts/fixture-to-pg.py mock/fixture.json | psql "$VIEWER_PG_URL"
    /// VIEWER_PG_URL=postgres://… cargo test -- --ignored the_store_answers_as_the_demo
    /// ```
    ///
    /// VIEWER_FIXTURE names another fixture, the same for both.
    #[test]
    #[ignore = "needs a PostgreSQL loaded with the fixture"]
    fn the_store_answers_as_the_demo() {
        let url = std::env::var("VIEWER_PG_URL").expect("VIEWER_PG_URL");
        let demo = crate::demo::Demo::load();
        let mut s = Settings::default();
        s.neo4j_url = url;
        let pool = Pool::default();
        let pg = Postgres { settings: &s, pool: &pool };
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let wrong = std::cell::RefCell::new(Vec::<String>::new());
        // Numbers compare by value: the store writes a float 1.0 as 1.
        let same = |what: String, a: &Value, b: &Value| {
            let equal = match (a.as_f64(), b.as_f64()) {
                (Some(x), Some(y)) => x == y,
                _ => a == b,
            };
            if !equal {
                wrong.borrow_mut().push(format!("{what}: demo {a} / pg {b}"));
            }
        };

        // The sessions and the counts the pages read off them.
        let ds = demo.sessions();
        let ps = rt.block_on(pg.sessions()).unwrap();
        let by_id = |v: &Value| -> std::collections::BTreeMap<String, Value> {
            v.as_array().unwrap().iter().map(|r| (r["id"].as_str().unwrap().to_owned(), r.clone())).collect()
        };
        let (ds, ps) = (by_id(&ds), by_id(&ps));
        assert_eq!(ds.keys().collect::<Vec<_>>(), ps.keys().collect::<Vec<_>>(), "the same sessions");
        for (id, d) in &ds {
            for k in ["undecided", "risky", "from_result", "mode", "client", "cwd", "intent", "prompt", "last_at", "created_at"] {
                same(format!("session {id} {k}"), &d[k], &ps[id][k]);
            }
            // The fixture's stored counts predate its edits: count its graph instead.
            let g = demo.graph(id).unwrap();
            let nodes = g["nodes"].as_array().unwrap();
            let calls: Vec<&Value> = nodes.iter().filter(|n| n["kind"] == "Call").collect();
            let with = |d: &str| calls.iter().filter(|n| n["props"]["decision"] == d).count();
            for (k, n) in [("nodes", nodes.len()), ("calls", calls.len()), ("denied", with("DENY")),
                           ("reviewed", with("REVIEW")), ("allowed", with("ALLOW"))] {
                same(format!("session {id} {k}"), &json!(n), &ps[id][k]);
            }
        }

        for id in ds.keys() {
            let d = demo.graph(id).unwrap();
            let p = rt.block_on(pg.graph(id)).unwrap();
            // Nodes: the same, in the same order, each fixture property the same in the store.
            let (dn, pn) = (d["nodes"].as_array().unwrap(), p["nodes"].as_array().unwrap());
            same(format!("graph {id} node count"), &json!(dn.len()), &json!(pn.len()));
            for (a, b) in dn.iter().zip(pn) {
                same(format!("graph {id} kind"), &a["kind"], &b["kind"]);
                for (k, v) in a["props"].as_object().unwrap() {
                    same(format!("graph {id} seq {} {k}", a["props"]["seq"]), v, &b["props"][k]);
                }
            }
            // Edges as sets.
            let edges = |g: &Value| -> std::collections::BTreeSet<String> {
                g["edges"].as_array().unwrap().iter().map(|e| {
                    format!("{}>{} {} {} {} {}", e["source"], e["target"], e["type"], e["props"]["via"], e["props"]["kind"], e["paragraph"])
                }).collect()
            };
            let (de, pe) = (edges(&d), edges(&p));
            for e in de.symmetric_difference(&pe) {
                wrong.borrow_mut().push(format!("graph {id} edge only in {}: {e}", if de.contains(e) { "demo" } else { "pg" }));
            }
            // Subgraphs, their members as sets.
            let subs = |g: &Value| -> Vec<(Value, Vec<i64>)> {
                g["subgraphs"].as_array().unwrap().iter().map(|x| {
                    let mut m: Vec<i64> = x["members"].as_array().unwrap().iter().filter_map(Value::as_i64).collect();
                    m.sort();
                    (x["props"]["number"].clone(), m)
                }).collect()
            };
            if subs(&d) != subs(&p) {
                wrong.borrow_mut().push(format!("graph {id} subgraphs: demo {:?} / pg {:?}", subs(&d), subs(&p)));
            }
            let tokens = |v: Option<&Value>| -> Vec<String> {
                let mut t: Vec<String> = v.and_then(Value::as_array).into_iter().flatten().map(Value::to_string).collect();
                t.sort();
                t
            };
            if tokens(d.get("tokens")) != tokens(p.get("tokens")) {
                wrong.borrow_mut().push(format!("graph {id} tokens differ"));
            }
            // A node's paragraphs.
            for n in dn {
                let seq = n["props"]["seq"].as_i64().unwrap();
                let dd = demo.node(id, seq);
                if dd.is_null() {
                    continue;
                }
                let pd = rt.block_on(pg.node(id, seq)).unwrap();
                same(format!("node {id}/{seq} kind"), &dd["kind"], &pd["kind"]);
                same(format!("node {id}/{seq} paragraphs"), &json!(dd["paragraphs"].as_array().map_or(0, Vec::len)),
                     &json!(pd["paragraphs"].as_array().map_or(0, Vec::len)));
            }
        }

        // Every Call's facts, by (session, seq).
        let facts = |v: Value| -> std::collections::BTreeMap<(String, i64), Value> {
            v.as_array().unwrap().iter().map(|r| ((r["session"].as_str().unwrap().to_owned(), r["seq"].as_i64().unwrap()), r.clone())).collect()
        };
        let (df, pf) = (facts(demo.call_facts()), facts(rt.block_on(pg.call_facts()).unwrap()));
        same("call facts count".into(), &json!(df.len()), &json!(pf.len()));
        // A fixture keeps a long payload cut short, no longer JSON: the demo reads nothing of it,
        // the store's cut still finds the fields.
        let whole = |session: &str, seq: i64| -> bool {
            demo.graph(session).ok().and_then(|g| {
                g["nodes"].as_array().and_then(|n| n.iter().find(|x| x["props"]["seq"] == json!(seq)).cloned())
            }).and_then(|n| n["props"]["payload_json"].as_str().map(|p| serde_json::from_str::<Value>(p).is_ok())).unwrap_or(true)
        };
        for (k, d) in &df {
            let fields: &[&str] = if whole(&k.0, k.1) {
                &["target", "tool", "reason", "prompt", "ran", "ran_at", "at", "decision", "mode", "rule", "link",
                  "worst", "irreversible", "client", "cwd"]
            } else {
                &["reason", "prompt", "ran", "ran_at", "at", "decision", "mode", "rule", "link", "worst", "irreversible",
                  "client", "cwd"]
            };
            for f in fields.iter().copied() {
                same(format!("fact {k:?} {f}"), &d[f], &pf.get(k).map_or(Value::Null, |p| p[f].clone()));
            }
        }

        // Every sensitive mention, by (session, seq, key, slot).
        let mentions = |v: Value| -> std::collections::BTreeMap<String, Value> {
            v.as_array().unwrap().iter().map(|r| (format!("{} {} {} {}", r["session"], r["seq"], r["key"], r["slot"]), r.clone())).collect()
        };
        let (dm, pm) = (mentions(demo.secrets()), mentions(rt.block_on(pg.secrets()).unwrap()));
        same("secret mentions".into(), &json!(dm.keys().collect::<Vec<_>>()), &json!(pm.keys().collect::<Vec<_>>()));
        for (k, d) in &dm {
            for f in ["node", "kind", "category", "text", "decision", "client", "at", "tool", "target", "ran"] {
                same(format!("secret {k} {f}"), &d[f], &pm.get(k).map_or(Value::Null, |p| p[f].clone()));
            }
        }

        // The comparison's Calls, expected from the demo's own graph (its stored answers predate
        // the fixture's edits) through the same `call_row`.
        let ids: Vec<String> = ds.keys().cloned().collect();
        let pc = rt.block_on(pg.calls(&ids)).unwrap();
        for id in &ids {
            let g = demo.graph(id).unwrap();
            let ran: std::collections::HashSet<i64> = g["edges"].as_array().unwrap().iter()
                .filter(|e| e["type"] == "RESULT_OF").filter_map(|e| e["target"].as_i64()).collect();
            let mut expected: Vec<(i64, Value)> = g["nodes"].as_array().unwrap().iter()
                .filter(|n| n["kind"] == "Call")
                .map(|n| {
                    let props = n["props"].as_object().cloned().unwrap_or_default();
                    let seq = props["seq"].as_i64().unwrap();
                    (props["order"].as_i64().unwrap_or(0), call_row(&props, json!(ran.contains(&seq))))
                })
                .collect();
            expected.sort_by_key(|e| e.0);
            let b = pc[id].as_array().cloned().unwrap_or_default();
            same(format!("calls {id} count"), &json!(expected.len()), &json!(b.len()));
            for ((_, x), y) in expected.iter().zip(&b) {
                for f in ["seq", "tool_name", "decision", "ran", "worst", "link", "score"] {
                    same(format!("calls {id} {} {f}", x["seq"]), &x[f], &y[f]);
                }
            }
        }
        let wrong = wrong.into_inner();
        assert!(wrong.is_empty(), "{} differences:\n{}", wrong.len(), wrong.join("\n"));
    }

    #[test]
    fn the_pg_match_agrees_with_the_neo4j_one() {
        let pg = regex::Regex::new(IRREVERSIBLE).unwrap();
        let neo = regex::Regex::new(crate::graph::IRREVERSIBLE_TRACE).unwrap();
        for t in [
            r#"{"deviation":{"irreversible":0.5}}"#,
            r#"{"deviation":{"irreversible":0.49,"x":1}}"#,
            r#"{"deviation":{"irreversible":1.0}}"#,
            r#"{"deviation":{"irreversible":1,"y":2}}"#,
            r#"{"unconfirmed_irreversible":0.9}"#,
        ] {
            assert_eq!(pg.is_match(t), neo.is_match(t), "{t}");
        }
    }
}
