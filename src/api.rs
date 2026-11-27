//! The app's data source: the engine's PostgreSQL store, Neo4j (engines before withhuman-core
//! #118), or the demo graph, chosen by the settings. The screens call
//! these functions directly and get typed answers.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::demo::Demo;
use crate::graph::{ApiError, Neo4j};
use crate::model::{GraphData, NodeDetail, Session};
use crate::settings::{Change, Engine, Public, Settings};

pub struct Backend {
    client: reqwest::Client,
    settings: Mutex<Settings>,
    path: PathBuf,
    demo: Mutex<Demo>,
    /// The PostgreSQL connection kept between reads.
    pg: crate::pg::Pool,
}

/// What a settings test found: how many sessions the graph holds, and how long it took.
#[derive(Clone, Debug, PartialEq)]
pub struct Probe {
    pub sessions: i64,
    pub ms: u128,
}

/// How the app was started: `--onboard` shows the first launch's setup again, `--developer` starts
/// it on the company's engine (a developer's machine) rather than an engine of its own.
#[derive(Clone, Copy, Debug, Default)]
pub struct Launch {
    pub onboard: bool,
    pub developer: bool,
}

static LAUNCH: std::sync::OnceLock<Launch> = std::sync::OnceLock::new();

/// Reads the command line once, at start.
pub fn set_launch(args: impl Iterator<Item = String>) {
    let mut launch = Launch::default();
    for arg in args {
        match arg.as_str() {
            "--onboard" => launch.onboard = true,
            "--developer" => launch.developer = true,
            _ => {}
        }
    }
    let _ = LAUNCH.set(launch);
}

pub fn launch() -> Launch {
    LAUNCH.get().copied().unwrap_or_default()
}

/// The app's identifier: its settings folder and its keychain service. The two apps have their
/// own, so both can be installed on one machine without sharing settings or passwords.
#[cfg(feature = "admin")]
pub const APP_ID: &str = "dev.withhuman.viewer";
#[cfg(not(feature = "admin"))]
pub const APP_ID: &str = "dev.withhuman.personal";
/// What the personal app was kept under before it had its own, shared with the security team's.
#[cfg(not(feature = "admin"))]
pub const EARLIER_APP_ID: &str = "dev.withhuman.viewer";

/// The folder the app keeps its files in. The personal app's first run copies what it kept in
/// the shared folder before: the copy, so hooks installed from there go on working.
pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    let dir = base.join(APP_ID);
    // Not from a test run: the machine's real settings are not a test's to copy.
    #[cfg(not(any(feature = "admin", test)))]
    {
        let earlier = base.join(EARLIER_APP_ID);
        if !dir.exists()
            && earlier.is_dir()
            && let Err(e) = copy_dir(&earlier, &dir)
        {
            eprintln!("withhuman: could not copy the earlier settings: {e}");
        }
    }
    dir
}

#[cfg(not(any(feature = "admin", test)))]
fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

impl Backend {
    pub fn load() -> Self {
        let path = config_dir().join("settings.json");
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("HTTP client");
        Self {
            client,
            settings: Mutex::new(match std::env::var("VIEWER_DB") {
                Ok(url) if !url.trim().is_empty() => Settings::load(&path).pointed_at(&url),
                _ => Settings::load(&path),
            }),
            path,
            demo: Mutex::new(Demo::load()),
            pg: crate::pg::Pool::default(),
        }
    }

    /// The settings to read with. For Neo4j the password comes out of the keychain the first
    /// time it is needed, and stays in memory after.
    fn current(&self) -> Settings {
        let mut settings = self.settings.lock().expect("settings lock");
        if !settings.demo && settings.password.is_empty() {
            *settings = settings.clone().unlocked();
        }
        settings.clone()
    }

    pub async fn sessions(&self) -> Result<Vec<Session>, String> {
        let settings = self.current();
        let value = if settings.demo {
            Ok(self.demo.lock().expect("demo lock").sessions())
        } else {
            if crate::pg::is_postgres(&settings) {
                let pg = crate::pg::Postgres {
                    settings: &settings,
                    pool: &self.pg,
                };
                // What was read is kept in a file beside the settings, and the next read asks only
                // for the sessions begun or with a Call seen since a little before the latest kept.
                let path = sessions_cache(&settings.neo4j_url);
                let kept: Vec<Value> = std::fs::read(&path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                    .unwrap_or_default();
                let at = |row: &Value| row["last_at"].as_i64().or(row["created_at"].as_i64());
                let since = kept
                    .iter()
                    .filter_map(at)
                    .max()
                    .map(|t| t - SESSIONS_OVERLAP_MS);
                // With what was kept, the IDs of all there are, to drop those gone since.
                let (fresh, ids) = tokio::join!(pg.sessions(since), async {
                    match since {
                        Some(_) => pg.session_ids().await.ok(),
                        None => None,
                    }
                });
                fresh.map(|fresh| {
                    let mut all = match fresh {
                        Value::Array(rows) => rows,
                        _ => Vec::new(),
                    };
                    let read: std::collections::HashSet<String> = all
                        .iter()
                        .filter_map(|r| r["id"].as_str().map(str::to_owned))
                        .collect();
                    if since.is_some() {
                        let there: Option<std::collections::HashSet<String>> =
                            ids.map(|ids| ids.into_iter().collect());
                        all.extend(kept.into_iter().filter(|r| {
                            r["id"].as_str().is_some_and(|id| {
                                !read.contains(id) && there.as_ref().is_none_or(|t| t.contains(id))
                            })
                        }));
                    }
                    all.sort_by_key(|r| std::cmp::Reverse(r["created_at"].as_i64()));
                    all.truncate(SESSIONS_KEPT);
                    if let Ok(text) = serde_json::to_vec(&all) {
                        let _ = std::fs::write(&path, text);
                    }
                    Value::Array(all)
                })
            } else {
                Neo4j {
                    client: &self.client,
                    settings: &settings,
                }
                .sessions()
                .await
            }
        };
        let mut sessions: Vec<Session> = typed(value)?;
        // A session opened and closed with nothing asked or done in it is not a session to read.
        sessions.retain(|s| !s.empty());
        // A client ID no setup registered is named by the folders its sessions ran in.
        crate::machines::learn_folders(&sessions);
        Ok(sessions)
    }

    pub async fn graph(&self, session: &str) -> Result<GraphData, String> {
        let settings = self.current();
        let value = if settings.demo {
            self.demo.lock().expect("demo lock").graph(session)
        } else {
            if crate::pg::is_postgres(&settings) {
                crate::pg::Postgres {
                    settings: &settings,
                    pool: &self.pg,
                }
                .graph(session)
                .await
            } else {
                Neo4j {
                    client: &self.client,
                    settings: &settings,
                }
                .graph(session)
                .await
            }
        };
        typed(value)
    }

    pub async fn node(&self, session: &str, seq: i64) -> Result<Option<NodeDetail>, String> {
        let settings = self.current();
        let value = if settings.demo {
            Ok(self.demo.lock().expect("demo lock").node(session, seq))
        } else {
            if crate::pg::is_postgres(&settings) {
                crate::pg::Postgres {
                    settings: &settings,
                    pool: &self.pg,
                }
                .node(session, seq)
                .await
            } else {
                Neo4j {
                    client: &self.client,
                    settings: &settings,
                }
                .node(session, seq)
                .await
            }
        };
        typed(value)
    }

    /// Every Call, cut down to what the security team's charts count.
    ///
    /// Reading them all from PostgreSQL takes long (each Call's tool and target are cut out of
    /// its JSON there), so what was read is kept in a file beside the settings and the next
    /// read asks only for the Calls since a day before the newest kept. `full` reads them all
    /// again, as the reload button does, in case older Calls changed or were removed.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn call_facts(&self, full: bool) -> Result<Vec<crate::model::CallFact>, String> {
        let settings = self.current();
        let value = if settings.demo {
            Ok(self.demo.lock().expect("demo lock").call_facts())
        } else {
            if crate::pg::is_postgres(&settings) {
                let pg = crate::pg::Postgres {
                    settings: &settings,
                    pool: &self.pg,
                };
                let path = facts_cache(&settings.neo4j_url);
                let kept: Vec<Value> = if full {
                    Vec::new()
                } else {
                    std::fs::read(&path)
                        .ok()
                        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                        .unwrap_or_default()
                };
                let at = |row: &Value| row["at"].as_i64();
                let since = kept
                    .iter()
                    .filter_map(at)
                    .max()
                    .map(|t| t - FACTS_OVERLAP_MS);
                // With what was kept, the IDs of the sessions there are now: a session removed
                // from the store (or the store emptied) takes its kept Calls with it.
                let (fresh, ids) = tokio::join!(pg.call_facts(since), async {
                    match since {
                        Some(_) => pg.session_ids().await.ok(),
                        None => None,
                    }
                });
                fresh.map(|fresh| {
                    let mut all = match fresh {
                        Value::Array(rows) => rows,
                        _ => Vec::new(),
                    };
                    if let Some(since) = since {
                        let there: Option<std::collections::HashSet<String>> =
                            ids.map(|ids| ids.into_iter().collect());
                        all.extend(kept.into_iter().filter(|r| {
                            at(r).is_none_or(|t| t < since)
                                && there.as_ref().is_none_or(|there| {
                                    r["session"].as_str().is_some_and(|s| there.contains(s))
                                })
                        }));
                    }
                    all.sort_by_key(|r| std::cmp::Reverse(at(r)));
                    all.truncate(FACTS_KEPT);
                    if let Ok(text) = serde_json::to_vec(&all) {
                        let _ = std::fs::write(&path, text);
                    }
                    Value::Array(all)
                })
            } else {
                Neo4j {
                    client: &self.client,
                    settings: &settings,
                }
                .call_facts()
                .await
            }
        };
        typed(value)
    }

    /// Every mention of a sensitive value, for the security team's 민감 정보.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn secrets(&self) -> Result<Vec<crate::model::SecretMention>, String> {
        let settings = self.current();
        let value = if settings.demo {
            Ok(self.demo.lock().expect("demo lock").secrets())
        } else {
            if crate::pg::is_postgres(&settings) {
                crate::pg::Postgres {
                    settings: &settings,
                    pool: &self.pg,
                }
                .secrets()
                .await
            } else {
                Neo4j {
                    client: &self.client,
                    settings: &settings,
                }
                .secrets()
                .await
            }
        };
        typed(value)
    }

    /// The engine's agent inventory; none when the data has none (Neo4j, or an engine that keeps
    /// no inventory yet).
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn inventory(&self) -> Result<Option<crate::model::Inventory>, String> {
        let settings = self.current();
        if settings.demo {
            let inventory = self.demo.lock().expect("demo lock").inventory(now_ms());
            crate::machines::learn(&inventory.clients);
            return Ok(Some(inventory));
        }
        if !crate::pg::is_postgres(&settings) {
            return Ok(None);
        }
        let value = crate::pg::Postgres {
            settings: &settings,
            pool: &self.pg,
        }
        .inventory()
        .await;
        match value {
            Ok(Value::Null) => Ok(None),
            other => typed::<crate::model::Inventory>(other).map(|inventory| {
                // The pages name a client ID's machine from what the inventory says of it.
                crate::machines::learn(&inventory.clients);
                Some(inventory)
            }),
        }
    }

    /// Keeps the engine's address for key actions, and a new admin token in the keychain.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub fn save_admin(&self, engine_api: &str, token: Option<&str>) -> Result<Public, String> {
        let mut settings = self.settings.lock().expect("settings lock");
        let mut next = settings.with_admin(engine_api, token)?;
        next.save(&self.path)?;
        *settings = next;
        Ok(settings.public(&self.path))
    }

    /// What the security team marked and distrusts: the engine's, where it keeps them for
    /// everyone, else this Mac's file. What the file held before the engine kept them is handed
    /// to the engine once.
    #[cfg(feature = "admin")]
    pub async fn triage(&self) -> crate::admin::triage::Store {
        use crate::admin::triage::{self, Store};
        let settings = self.current();
        if settings.demo || !crate::pg::is_postgres(&settings) {
            return triage::load();
        }
        let pg = crate::pg::Postgres {
            settings: &settings,
            pool: &self.pg,
        };
        let Ok(found) = pg.triage().await else {
            return triage::load();
        };
        if found.is_null() {
            return triage::load();
        }
        let local = triage::load();
        if local.marks.is_empty() && local.sources.is_empty() {
            return Store::from_engine(&found);
        }
        // Hand the file's over, the marks by state; set the file aside only when all went.
        let mut ok = true;
        for state in ["reviewing", "attack", "benign", "closed"] {
            let sessions: Vec<&str> = local
                .marks
                .iter()
                .filter(|(_, m)| m.state == state)
                .map(|(s, _)| s.as_str())
                .collect();
            for chunk in sessions.chunks(500) {
                let by = local.marks[chunk[0]].by.clone();
                ok &= self
                    .admin(
                        &["v1", "admin", "sessions", "mark"],
                        json!({"session_ids": chunk, "state": state, "actor": by}),
                    )
                    .await
                    .is_ok();
            }
        }
        for b in &local.sources {
            ok &= self
                .admin(
                    &["v1", "admin", "sources", "distrust"],
                    json!({
                        "source": b.source, "on": true, "actor": b.by,
                        "session_id": Some(&b.session).filter(|s| !s.is_empty()), "seq": b.seq,
                    }),
                )
                .await
                .is_ok();
        }
        if ok {
            triage::set_aside();
        }
        let mut store = match pg.triage().await {
            Ok(v) if !v.is_null() => Store::from_engine(&v),
            _ => Store::from_engine(&found),
        };
        // What the engine would not take yet still shows, this Mac's over the engine's.
        if !ok {
            store.marks.extend(local.marks);
            for b in local.sources {
                if !store.sources.iter().any(|x| x.source == b.source) {
                    store.sources.push(b);
                }
            }
        }
        store
    }

    /// Marks sessions (`state` none takes the mark off): in the engine where it keeps marks,
    /// else in this Mac's file.
    #[cfg(feature = "admin")]
    pub async fn mark_sessions(
        &self,
        shared: bool,
        sessions: &[String],
        state: Option<&str>,
        actor: &str,
    ) -> Result<(), String> {
        if shared {
            let mut taken = true;
            for chunk in sessions.chunks(500) {
                taken &= self
                    .admin(
                        &["v1", "admin", "sessions", "mark"],
                        json!({"session_ids": chunk, "state": state, "actor": actor}),
                    )
                    .await
                    .is_ok();
            }
            if taken {
                return Ok(());
            }
        }
        // Where the engine keeps no marks or would not take them: on this Mac, handed over
        // when it does.
        let mut store = crate::admin::triage::load();
        for s in sessions {
            crate::admin::triage::mark(&mut store, s, state, actor, now_ms());
        }
        Ok(())
    }

    /// Distrusts a source, or trusts it again: in the engine where it keeps them, else in this
    /// Mac's file.
    #[cfg(feature = "admin")]
    pub async fn distrust(
        &self,
        shared: bool,
        source: &str,
        on: bool,
        actor: &str,
        call: Option<(String, i64)>,
    ) -> Result<(), String> {
        if !shared {
            let mut store = crate::admin::triage::load();
            let blocked = on.then(|| {
                let (session, seq) = call.unwrap_or_default();
                crate::admin::triage::Blocked {
                    source: source.to_owned(),
                    by: actor.to_owned(),
                    at: now_ms(),
                    session,
                    seq,
                }
            });
            crate::admin::triage::block(&mut store, source, blocked);
            return Ok(());
        }
        let (session, seq) = call.unzip();
        self.admin(
            &["v1", "admin", "sources", "distrust"],
            json!({"source": source, "on": on, "actor": actor, "session_id": session, "seq": seq}),
        )
        .await
        .map(|_| ())
    }

    /// Stops a machine (every tool call from it is refused) or lets it go again. The engine's
    /// admin API keeps it (`POST /v1/admin/clients/{id}/stop`). The machine names itself, so
    /// this holds only while its hooks keep sending its ID.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn stop_device(
        &self,
        client: &str,
        stopped: bool,
        actor: &str,
        reason: &str,
    ) -> Result<(), String> {
        if self.current().demo {
            let found = self.demo.lock().expect("demo lock").stop_device(
                now_ms(),
                client,
                stopped,
                actor,
                reason,
            );
            return if found {
                Ok(())
            } else {
                Err("그런 기기가 없습니다.".into())
            };
        }
        self.admin(
            &["v1", "admin", "clients", client, "stop"],
            json!({"stopped": stopped, "actor": actor, "reason": reason}),
        )
        .await
        .map(|_| ())
    }

    /// The MCP servers the machines have set up and their tools' calls; none until the engine
    /// collects them, so only the demo has them yet.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn mcp_catalog(&self) -> Result<Option<crate::model::McpCatalog>, String> {
        if self.current().demo {
            return Ok(Some(self.demo.lock().expect("demo lock").mcp(now_ms())));
        }
        Ok(None)
    }

    /// One call of the engine's admin API (Collector's /v1/admin): a POST with the admin token.
    async fn admin(&self, path: &[&str], body: Value) -> Result<Value, String> {
        self.admin_call(reqwest::Method::POST, path, Some(body))
            .await
    }

    /// Deletes a machine from the inventory with every session it ran
    /// (`DELETE /v1/admin/clients/{id}`); the number of its sessions deleted. It cannot be
    /// undone, and a machine that sends hooks again is recorded again.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn delete_device(&self, client: &str) -> Result<u64, String> {
        if self.current().demo {
            return self
                .demo
                .lock()
                .expect("demo lock")
                .delete_device(now_ms(), client)
                .ok_or_else(|| "그런 기기가 없습니다.".into());
        }
        let answer = self
            .admin_call(
                reqwest::Method::DELETE,
                &["v1", "admin", "clients", client],
                None,
            )
            .await?;
        Ok(answer["sessions"].as_u64().unwrap_or(0))
    }

    /// Whether the engine's admin API can be called: the demo, or an address and a token kept.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub fn can_admin(&self) -> bool {
        let settings = self.current();
        settings.demo || settings.admin_token().is_some() && !settings.engine_api.trim().is_empty()
    }

    /// One call of the engine's admin API with the admin token: `method` at `path`, with a JSON
    /// `body` when there is one.
    async fn admin_call(
        &self,
        method: reqwest::Method,
        path: &[&str],
        body: Option<Value>,
    ) -> Result<Value, String> {
        let settings = self.current();
        let token = settings.admin_token().ok_or("엔진 관리 토큰이 없습니다.")?;
        let mut url = reqwest::Url::parse(&settings.engine_api)
            .map_err(|e| format!("엔진 관리 API 주소를 읽지 못했습니다: {e}"))?;
        url.path_segments_mut()
            .map_err(|_| "엔진 관리 API 주소가 올바르지 않습니다.".to_owned())?
            .pop_if_empty()
            .extend(path);
        let mut request = self.client.request(method, url).bearer_auth(token);
        if let Some(body) = &body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|e| {
            format!(
                "엔진 관리 API({})에 닿지 않습니다: {e}",
                settings.engine_api
            )
        })?;
        let status = response.status().as_u16();
        let answer: Value = response.json().await.unwrap_or(Value::Null);
        match status {
            200..=299 => Ok(answer),
            401 | 403 => Err("엔진이 관리 토큰을 받지 않았습니다. 토큰을 다시 넣어 주세요.".into()),
            404 => Err("엔진에 관리 API가 꺼져 있거나 대상이 없습니다(엔진의 WITHHUMAN_ADMIN_TOKEN 설정을 확인해 주세요).".into()),
            _ => Err(format!(
                "엔진이 거절했습니다 (HTTP {status}): {}",
                answer.get("error").and_then(Value::as_str).unwrap_or_default()
            )),
        }
    }

    /// Each session's Calls, for the comparison.
    pub async fn calls(
        &self,
        sessions: &[String],
    ) -> Result<std::collections::HashMap<String, Vec<crate::model::CallRow>>, String> {
        let settings = self.current();
        let value = if settings.demo {
            Ok(self.demo.lock().expect("demo lock").calls(sessions))
        } else {
            if crate::pg::is_postgres(&settings) {
                crate::pg::Postgres {
                    settings: &settings,
                    pool: &self.pg,
                }
                .calls(sessions)
                .await
            } else {
                Neo4j {
                    client: &self.client,
                    settings: &settings,
                }
                .calls(sessions)
                .await
            }
        };
        typed(value)
    }

    /// Deletes a session for good; returns how many nodes went with it.
    pub async fn delete(&self, session: &str) -> Result<i64, String> {
        let settings = self.current();
        let value = if settings.demo {
            self.demo.lock().expect("demo lock").delete(session)
        } else {
            if crate::pg::is_postgres(&settings) {
                crate::pg::Postgres {
                    settings: &settings,
                    pool: &self.pg,
                }
                .delete_session(session)
                .await
            } else {
                Neo4j {
                    client: &self.client,
                    settings: &settings,
                }
                .delete_session(session)
                .await
            }
        };
        if value.is_ok() && crate::pg::is_postgres(&settings) {
            let path = sessions_cache(&settings.neo4j_url);
            if let Some(mut kept) = std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Vec<Value>>(&bytes).ok())
            {
                kept.retain(|r| r["id"].as_str() != Some(session));
                if let Ok(text) = serde_json::to_vec(&kept) {
                    let _ = std::fs::write(&path, text);
                }
            }
        }
        value
            .map(|v| v["deleted"].as_i64().unwrap_or(0))
            .map_err(|e| e.error)
    }

    pub fn settings(&self) -> Public {
        self.settings
            .lock()
            .expect("settings lock")
            .public(&self.path)
    }

    pub fn save(&self, change: Change) -> Result<Public, String> {
        let mut settings = self.settings.lock().expect("settings lock");
        let mut next = settings.apply(change)?;
        next.save(&self.path)?;
        *settings = next;
        Ok(settings.public(&self.path))
    }

    /// Saves a store on a server as where the record is read from, in place of the app's own
    /// engine if it was that.
    #[cfg(not(feature = "admin"))]
    pub fn save_server(&self, change: Change) -> Result<Public, String> {
        let mut settings = self.settings.lock().expect("settings lock");
        let mut next = settings.apply(change)?;
        next.engine = Engine::Server;
        next.save(&self.path)?;
        *settings = next;
        Ok(settings.public(&self.path))
    }

    /// Whether the first launch's setup is still to do: the security team's app has no Jev key
    /// registered, the developer's no settings saved yet. `VIEWER_ONBOARD=1` shows it again; the
    /// demo (`VIEWER_MOCK=1`) skips the security team's.
    pub fn needs_setup(&self) -> bool {
        if std::env::var("VIEWER_ONBOARD").is_ok_and(|v| v == "1") || launch().onboard {
            return true;
        }
        if !cfg!(feature = "admin") {
            return self.settings.lock().expect("settings lock").first_run
                && std::env::var("VIEWER_DB").is_err();
        }
        if std::env::var("VIEWER_MOCK").is_ok_and(|v| v == "1")
            || std::env::var("VIEWER_DB").is_ok()
        {
            return false;
        }
        !self.settings.lock().expect("settings lock").jev_saved
    }

    /// Asks Jev whether it takes the key.
    pub async fn check_jev(&self, key: &str) -> Result<(), String> {
        crate::engine::check_jev(&self.client, key).await
    }

    /// The Jev models the key can ask, the engine's own default first.
    #[cfg(not(feature = "admin"))]
    pub async fn jev_models(&self, key: &str) -> Vec<String> {
        let mut models = vec![crate::own::DEFAULT_MODEL.to_owned()];
        if let Ok(served) = crate::engine::jev_models(&self.client, key).await {
            for m in served {
                if !models.contains(&m) {
                    models.push(m);
                }
            }
        }
        models
    }

    /// Takes the personal app off this machine, all but the app itself: every WithHuman hook
    /// setup it finds (`folders`, the ones it installed in, besides the user's and the device's),
    /// the engine with its record, the app's secrets and its settings folder. The app quits
    /// after.
    #[cfg(not(feature = "admin"))]
    pub fn remove_everything(&self, folders: &[String]) -> Result<(), String> {
        crate::hooks::reset(folders)?;
        crate::own::uninstall()?;
        let mut accounts: Vec<&str> = crate::own::ACCOUNTS.to_vec();
        let store = self
            .settings
            .lock()
            .expect("settings lock")
            .keychain_account();
        accounts.push(&store);
        crate::settings::forget_secrets(&accounts)?;
        match std::fs::remove_dir_all(config_dir()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("설정 폴더를 지우지 못했습니다: {e}")),
        }
    }

    /// Starts the personal app's own engine, with the Jev key (kept when given; none keeps the
    /// one kept before, or none at all) and model, sending Calls to Jev or not (`jev`), then
    /// waits for every part to answer. Gives the connection the record is read with: the
    /// engine's store, with the viewer's read-only login.
    #[cfg(not(feature = "admin"))]
    pub async fn start_own(&self, key: &str, model: &str, jev: bool) -> Result<Change, String> {
        if !key.trim().is_empty() {
            crate::settings::save_jev_key(key)?;
        }
        crate::own::Config {
            model: model.to_owned(),
            jev,
        }
        .save()?;
        let (_, viewer) = crate::own::passwords()?;
        let running = crate::own::health().await.iter().all(|(_, up)| *up);
        tokio::task::spawn_blocking(move || {
            if running {
                crate::own::restart()
            } else {
                crate::own::ensure_started()
            }
        })
        .await
        .map_err(|e| e.to_string())??;
        // A restart takes a moment to stop the old engine first.
        tokio::time::sleep(Duration::from_secs(2)).await;
        for _ in 0..180 {
            if crate::own::health().await.iter().all(|(_, up)| *up) {
                return Ok(Change {
                    neo4j_url: crate::own::store_url(),
                    database: "withhuman".into(),
                    user: crate::own::VIEWER_USER.into(),
                    password: Some(viewer),
                    demo: false,
                });
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Err(format!(
            "엔진이 3분 안에 다 뜨지 않았습니다. 로그: {}",
            crate::own::logs().display()
        ))
    }

    /// Keeps the developer's own engine as where the record is read from and the hooks report.
    #[cfg(not(feature = "admin"))]
    pub fn finish_own(&self, change: Change) -> Result<Public, String> {
        let mut settings = self.settings.lock().expect("settings lock");
        let mut next = settings.apply(change)?;
        next.engine = Engine::Own;
        next.engine_api = crate::own::collector_url();
        next.jev_saved = true;
        next.demo = false;
        next.save(&self.path)?;
        *settings = next;
        Ok(settings.public(&self.path))
    }

    /// Starts the engine in `dir` with the Jev key, then waits for its Neo4j to answer. Docker's
    /// lines go to `log` as they come. Gives the connection the viewer reads it with.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn start_local(
        &self,
        dir: std::path::PathBuf,
        key: String,
        log: std::sync::Arc<Mutex<Vec<String>>>,
    ) -> Result<Change, String> {
        let local = crate::engine::Local::read(&dir)?;
        let change = Change {
            neo4j_url: format!("http://127.0.0.1:{}", local.port),
            database: "neo4j".into(),
            user: "neo4j".into(),
            password: Some(local.password.clone()),
            demo: false,
        };
        let l = log.clone();
        tokio::task::spawn_blocking(move || local.up(&key, &l))
            .await
            .map_err(|e| e.to_string())??;
        if let Ok(mut l) = log.lock() {
            l.push("Neo4j가 응답하기를 기다리는 중…".into());
        }
        let mut last = String::new();
        for _ in 0..60 {
            match self.test(clone_change(&change)).await {
                Ok(probe) => {
                    if let Ok(mut l) = log.lock() {
                        l.push(format!("Neo4j 연결됨 · 세션 {}개", probe.sessions));
                    }
                    return Ok(change);
                }
                Err(e) => last = e,
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        Err(format!(
            "엔진은 떴지만 Neo4j가 2분 안에 응답하지 않았습니다. ({last})"
        ))
    }

    /// Ends the first launch's setup: keeps the Jev key in the keychain and the connection
    /// in the settings, with where the engine runs.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub fn finish_setup(
        &self,
        key: &str,
        change: Change,
        engine: Engine,
        dir: Option<String>,
    ) -> Result<Public, String> {
        crate::settings::save_jev_key(key)?;
        let mut settings = self.settings.lock().expect("settings lock");
        let mut next = settings.apply(change)?;
        next.engine = engine;
        next.engine_dir = dir;
        next.jev_saved = true;
        next.demo = false;
        next.save(&self.path)?;
        *settings = next;
        Ok(settings.public(&self.path))
    }

    /// Switches between the demo graph and Neo4j, keeping the connection as it is.
    pub fn set_demo(&self, demo: bool) -> Result<Public, String> {
        let mut settings = self.settings.lock().expect("settings lock");
        let mut next = settings.clone();
        next.demo = demo;
        next.save(&self.path)?;
        *settings = next;
        Ok(settings.public(&self.path))
    }

    /// Tries settings from the form before they are saved: connects and counts the sessions.
    pub async fn test(&self, change: Change) -> Result<Probe, String> {
        let candidate = self
            .settings
            .lock()
            .expect("settings lock")
            .apply(change)?
            .unlocked();
        let started = Instant::now();
        if crate::pg::is_postgres(&candidate) {
            let pool = crate::pg::Pool::default();
            let sessions = crate::pg::Postgres {
                settings: &candidate,
                pool: &pool,
            }
            .count_sessions()
            .await
            .map_err(|e| e.error)?;
            return Ok(Probe {
                sessions,
                ms: started.elapsed().as_millis(),
            });
        }
        let db = Neo4j {
            client: &self.client,
            settings: &candidate,
        };
        let rows = db
            .cypher(
                "MATCH (s:Session) RETURN count(s) AS sessions",
                "READ",
                json!({}),
            )
            .await
            .map_err(|e| e.error)?;
        let sessions = rows
            .first()
            .and_then(|r| r.get("sessions"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        Ok(Probe {
            sessions,
            ms: started.elapsed().as_millis(),
        })
    }
}

#[cfg_attr(not(feature = "admin"), allow(dead_code))]
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// A form's change again, for one more try.
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
fn clone_change(c: &Change) -> Change {
    Change {
        neo4j_url: c.neo4j_url.clone(),
        database: c.database.clone(),
        user: c.user.clone(),
        password: c.password.clone(),
        demo: c.demo,
    }
}

/// The Calls read again on every read: those of the last day before the newest kept, whose
/// decisions and results may still have come in.
const FACTS_OVERLAP_MS: i64 = 24 * 60 * 60 * 1000;
/// As many Calls as one read of the store hands over.
const FACTS_KEPT: usize = 50_000;

/// How far before the latest session kept the next read of the sessions starts, for Calls
/// decided after they were seen.
const SESSIONS_OVERLAP_MS: i64 = 10 * 60 * 1000;
/// As many sessions as one read of the store hands over.
const SESSIONS_KEPT: usize = 5_000;

/// The file a store's sessions are kept in between reads, named after the store's address.
fn sessions_cache(url: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    // The number is the rows' shape: a file of an older shape is left unread. 2: a session's
    // client ID falls back to the session's own.
    config_dir().join(format!("sessions2-{:016x}.json", hasher.finish()))
}

/// The file a store's Calls are kept in between reads, named after the store's address.
fn facts_cache(url: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    // The number is the rows' shape: a file of an older shape is left unread.
    config_dir().join(format!("facts2-{:016x}.json", hasher.finish()))
}

fn typed<T: DeserializeOwned>(value: Result<Value, ApiError>) -> Result<T, String> {
    let value = value.map_err(|e| e.error)?;
    serde_json::from_value(value).map_err(|e| format!("응답 형식이 예상과 다릅니다: {e}"))
}
