//! What the security team decided about what it looked at. An engine that keeps it
//! (`session_marks`, `distrusted_sources`) shares it with the whole team, and the app reads and
//! writes it there ([`crate::api::Backend::triage`]); for the demo and an older engine it is
//! kept in a file beside the settings (`triage.json`) on this Mac: each session marked
//! as being looked at, an attack, nothing wrong or closed, and the sources (a document, a URL, a tool) it chose to distrust.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A session's mark (`reviewing`, `attack`, `benign` or `closed`), who set it and when (ms).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub state: String,
    pub by: String,
    pub at: i64,
}

/// A source the team distrusts: what it is, who said so and when, and the Call that showed it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blocked {
    pub source: String,
    pub by: String,
    pub at: i64,
    pub session: String,
    pub seq: i64,
}

/// Everything kept: the marks by session, and the distrusted sources, the newest first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub marks: HashMap<String, Mark>,
    pub sources: Vec<Blocked>,
    /// Kept by the engine for everyone, not in this Mac's file.
    #[serde(skip)]
    pub shared: bool,
}

impl Store {
    /// The engine's rows (`Postgres::triage`) as a store.
    pub fn from_engine(v: &serde_json::Value) -> Self {
        let text = |r: &serde_json::Value, k: &str| r[k].as_str().unwrap_or_default().to_owned();
        Store {
            marks: v["marks"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| {
                    (
                        text(r, "session_id"),
                        Mark {
                            state: text(r, "state"),
                            by: text(r, "set_by"),
                            at: r["set_at"].as_i64().unwrap_or(0),
                        },
                    )
                })
                .collect(),
            sources: v["sources"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| Blocked {
                    source: text(r, "source"),
                    by: text(r, "set_by"),
                    at: r["set_at"].as_i64().unwrap_or(0),
                    session: text(r, "session_id"),
                    seq: r["seq"].as_i64().unwrap_or(0),
                })
                .collect(),
            shared: true,
        }
    }
}

fn path() -> std::path::PathBuf {
    crate::api::config_dir().join("triage.json")
}

/// What the file holds; nothing when it is missing or cannot be read. A file of marks alone,
/// as it first was, is read as such.
pub fn load() -> Store {
    let Ok(bytes) = std::fs::read(path()) else {
        return Store::default();
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    if value.get("marks").is_some() || value.get("sources").is_some() {
        serde_json::from_value(value).unwrap_or_default()
    } else {
        Store {
            marks: serde_json::from_value(value).unwrap_or_default(),
            sources: Vec::new(),
            shared: false,
        }
    }
}

fn save(store: &Store) {
    let path = path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_vec_pretty(store) {
        let _ = std::fs::write(path, text);
    }
}

/// Sets a session's mark, or takes it off (`state` none), and writes the file.
pub fn mark(store: &mut Store, session: &str, state: Option<&str>, by: &str, at: i64) {
    match state.filter(|s| matches!(*s, "reviewing" | "attack" | "benign" | "closed")) {
        Some(state) => {
            store.marks.insert(
                session.to_owned(),
                Mark {
                    state: state.to_owned(),
                    by: by.to_owned(),
                    at,
                },
            );
        }
        None => {
            store.marks.remove(session);
        }
    }
    save(store);
}

/// Distrusts a source, or trusts it again (`blocked` none), and writes the file.
pub fn block(store: &mut Store, source: &str, blocked: Option<Blocked>) {
    store.sources.retain(|b| b.source != source);
    if let Some(b) = blocked {
        store.sources.insert(0, b);
    }
    save(store);
}

/// The file's marks and sources set aside once they were handed to the engine, so they are
/// handed over only once.
pub fn set_aside() {
    let from = path();
    let _ = std::fs::rename(&from, from.with_file_name("triage.moved.json"));
}
