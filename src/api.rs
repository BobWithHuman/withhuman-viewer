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

/// The folder the app keeps its files in.
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("dev.withhuman.viewer")
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
                crate::pg::Postgres {
                    settings: &settings,
                    pool: &self.pg,
                }
                .sessions()
                .await
            } else {
                Neo4j {
                    client: &self.client,
                    settings: &settings,
                }
                .sessions()
                .await
            }
        };
        typed(value)
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
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn call_facts(&self) -> Result<Vec<crate::model::CallFact>, String> {
        let settings = self.current();
        let value = if settings.demo {
            Ok(self.demo.lock().expect("demo lock").call_facts())
        } else {
            if crate::pg::is_postgres(&settings) {
                crate::pg::Postgres {
                    settings: &settings,
                    pool: &self.pg,
                }
                .call_facts()
                .await
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
            return Ok(Some(
                self.demo.lock().expect("demo lock").inventory(now_ms()),
            ));
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
            other => typed(other).map(Some),
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

    /// Issues a key to a person: its id, and the key itself, shown this once.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn issue_key(
        &self,
        email: &str,
        actor: &str,
        note: &str,
    ) -> Result<(String, String), String> {
        if self.current().demo {
            return Ok(self.demo.lock().expect("demo lock").issue_key(
                now_ms(),
                email,
                actor,
                note,
            ));
        }
        let answer = self
            .admin(
                &["v1", "admin", "users", email, "keys"],
                json!({"actor": actor, "note": note}),
            )
            .await?;
        let text = |k: &str| {
            answer
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Ok((text("key_id"), text("key")))
    }

    /// Revokes one key, or every key of a person (the person's kill switch): how many were revoked.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn revoke(
        &self,
        key_id: Option<&str>,
        email: Option<&str>,
        actor: &str,
        reason: &str,
    ) -> Result<usize, String> {
        if self.current().demo {
            return Ok(self.demo.lock().expect("demo lock").revoke(
                now_ms(),
                key_id,
                email,
                actor,
                reason,
            ));
        }
        let body = json!({"actor": actor, "reason": reason});
        let answer = match (key_id, email) {
            (Some(id), _) => {
                self.admin(&["v1", "admin", "keys", id, "revoke"], body)
                    .await?
            }
            (None, Some(e)) => {
                self.admin(&["v1", "admin", "users", e, "revoke"], body)
                    .await?
            }
            (None, None) => return Ok(0),
        };
        Ok(answer.as_array().map_or(1, Vec::len))
    }

    /// The MCP servers and tools the gateway lets through, with the developers' requests and the
    /// calls it refused; none until the gateway keeps an MCP allowlist, which only the demo has
    /// yet.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn mcp_catalog(&self) -> Result<Option<crate::model::McpCatalog>, String> {
        if self.current().demo {
            return Ok(Some(self.demo.lock().expect("demo lock").mcp(now_ms())));
        }
        Ok(None)
    }

    /// Changes the gateway's MCP allowlist: the demo's only, until the gateway has an API for it.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub async fn mcp_change(&self, change: crate::model::McpChange) -> Result<(), String> {
        if self.current().demo {
            self.demo
                .lock()
                .expect("demo lock")
                .mcp_change(now_ms(), &change);
            return Ok(());
        }
        Err("게이트웨이에 MCP 허용 목록을 바꾸는 API가 아직 없습니다.".into())
    }

    /// One call of the engine's admin API (Collector's /v1/admin): a POST with the admin token.
    async fn admin(&self, path: &[&str], body: Value) -> Result<Value, String> {
        let settings = self.current();
        let token = settings
            .admin_token()
            .ok_or("엔진 관리 토큰이 없습니다. 인벤토리의 '관리 API 연결'에서 넣어 주세요.")?;
        let mut url = reqwest::Url::parse(&settings.engine_api)
            .map_err(|e| format!("엔진 관리 API 주소를 읽지 못했습니다: {e}"))?;
        url.path_segments_mut()
            .map_err(|_| "엔진 관리 API 주소가 올바르지 않습니다.".to_owned())?
            .pop_if_empty()
            .extend(path);
        let response = self
            .client
            .post(url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
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

    /// Whether the first launch's setup is still to do: no Jev key was registered.
    /// `VIEWER_ONBOARD=1` shows it again; the demo (`VIEWER_MOCK=1`) skips it.
    pub fn needs_setup(&self) -> bool {
        if std::env::var("VIEWER_ONBOARD").is_ok_and(|v| v == "1") {
            return true;
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

    /// Starts the engine in `dir` with the Jev key, then waits for its Neo4j to answer. Docker's
    /// lines go to `log` as they come. Gives the connection the viewer reads it with.
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
fn clone_change(c: &Change) -> Change {
    Change {
        neo4j_url: c.neo4j_url.clone(),
        database: c.database.clone(),
        user: c.user.clone(),
        password: c.password.clone(),
        demo: c.demo,
    }
}

fn typed<T: DeserializeOwned>(value: Result<Value, ApiError>) -> Result<T, String> {
    let value = value.map_err(|e| e.error)?;
    serde_json::from_value(value).map_err(|e| format!("응답 형식이 예상과 다릅니다: {e}"))
}
