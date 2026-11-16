//! What the security team decided about what it looked at, kept in a file beside the settings
//! (`triage.json`) on this Mac until the engine keeps it for the whole team: each session marked
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
