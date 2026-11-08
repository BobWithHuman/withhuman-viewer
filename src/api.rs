//! The app's data source: Neo4j or the demo graph, chosen by the settings. The screens call
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
            settings: Mutex::new(Settings::load(&path)),
            path,
            demo: Mutex::new(Demo::load()),
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
            Neo4j {
                client: &self.client,
                settings: &settings,
            }
            .sessions()
            .await
        };
        typed(value)
    }

    pub async fn graph(&self, session: &str) -> Result<GraphData, String> {
        let settings = self.current();
        let value = if settings.demo {
            self.demo.lock().expect("demo lock").graph(session)
        } else {
            Neo4j {
                client: &self.client,
                settings: &settings,
            }
            .graph(session)
            .await
        };
        typed(value)
    }

    pub async fn node(&self, session: &str, seq: i64) -> Result<Option<NodeDetail>, String> {
        let settings = self.current();
        let value = if settings.demo {
            Ok(self.demo.lock().expect("demo lock").node(session, seq))
        } else {
            Neo4j {
                client: &self.client,
                settings: &settings,
            }
            .node(session, seq)
            .await
        };
        typed(value)
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
            Neo4j {
                client: &self.client,
                settings: &settings,
            }
            .calls(sessions)
            .await
        };
        typed(value)
    }

    /// Deletes a session for good; returns how many nodes went with it.
    pub async fn delete(&self, session: &str) -> Result<i64, String> {
        let settings = self.current();
        let value = if settings.demo {
            self.demo.lock().expect("demo lock").delete(session)
        } else {
            Neo4j {
                client: &self.client,
                settings: &settings,
            }
            .delete_session(session)
            .await
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
        if std::env::var("VIEWER_MOCK").is_ok_and(|v| v == "1") {
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
        Err(format!("엔진은 떴지만 Neo4j가 2분 안에 응답하지 않았습니다. ({last})"))
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
