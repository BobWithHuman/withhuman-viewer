//! Where the viewer finds Neo4j, kept in the app's config folder between launches. The
//! password is kept in the system keychain, never in the file.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub neo4j_url: String,
    pub database: String,
    pub user: String,
    /// Read from a file written before the keychain was used, so it can be moved there.
    /// Otherwise filled from the keychain only when Neo4j is reached ([`Settings::unlocked`]),
    /// so launching, or the demo, never waits on a keychain prompt.
    #[serde(skip_serializing)]
    pub password: String,
    /// The keychain holds this account's password.
    pub password_saved: bool,
    /// A password typed in the settings, to be put in the keychain on save.
    #[serde(skip)]
    new_password: bool,
    /// Reads the bundled demo graph instead of Neo4j.
    pub demo: bool,
}

impl Default for Settings {
    /// The same defaults as withhuman-core's `server.py`, so its local `deploy/docker` stack answers.
    fn default() -> Self {
        let env =
            |key: &str, fallback: &str| std::env::var(key).unwrap_or_else(|_| fallback.to_owned());
        Self {
            neo4j_url: env("NEO4J_URL", "http://127.0.0.1:7474"),
            database: env("NEO4J_DATABASE", "neo4j"),
            user: env("NEO4J_USER", "neo4j"),
            password: env("NEO4J_PASSWORD", "withhuman-e2e"),
            password_saved: false,
            new_password: false,
            demo: std::env::var("VIEWER_MOCK").is_ok_and(|v| v == "1"),
        }
    }
}

/// What the page sees: everything but the password itself.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Public {
    pub neo4j_url: String,
    pub database: String,
    pub user: String,
    pub has_password: bool,
    pub demo: bool,
    pub path: String,
}

/// A change from the settings form; a missing password keeps the stored one.
#[derive(Deserialize)]
pub struct Change {
    pub neo4j_url: String,
    pub database: String,
    pub user: String,
    pub password: Option<String>,
    pub demo: bool,
}

/// The system keychain entry for one Neo4j account.
mod keychain {
    const SERVICE: &str = "dev.withhuman.viewer";

    fn entry(account: &str) -> Option<keyring::Entry> {
        keyring::Entry::new(SERVICE, account).ok()
    }

    pub fn get(account: &str) -> Option<String> {
        entry(account)?.get_password().ok()
    }

    pub fn set(account: &str, password: &str) -> Result<(), String> {
        let entry = entry(account).ok_or("키체인을 열지 못했습니다.")?;
        entry
            .set_password(password)
            .map_err(|e| format!("비밀번호를 키체인에 저장하지 못했습니다: {e}"))
    }
}

impl Settings {
    /// The keychain account: the same user on another server is another password.
    fn account(&self) -> String {
        format!("{}@{}/{}", self.user, self.neo4j_url, self.database)
    }

    pub fn public(&self, path: &Path) -> Public {
        Public {
            neo4j_url: self.neo4j_url.clone(),
            database: self.database.clone(),
            user: self.user.clone(),
            has_password: self.password_saved || !self.password.is_empty(),
            demo: self.demo,
            path: path.display().to_string(),
        }
    }

    pub fn apply(&self, change: Change) -> Result<Self, String> {
        let url = change.neo4j_url.trim().trim_end_matches('/').to_owned();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("Neo4j 주소는 http:// 또는 https://로 시작해야 합니다.".into());
        }
        if change.database.trim().is_empty() || change.user.trim().is_empty() {
            return Err("데이터베이스와 사용자 이름을 채워 주세요.".into());
        }
        let mut next = Self {
            neo4j_url: url,
            database: change.database.trim().to_owned(),
            user: change.user.trim().to_owned(),
            password: self.password.clone(),
            password_saved: self.password_saved,
            new_password: self.new_password,
            demo: change.demo,
        };
        match change.password {
            Some(password) => {
                next.password = password;
                next.new_password = true;
            }
            // The same password under another account: carried over from the keychain.
            None if next.account() != self.account() && self.password_saved => {
                next.password = self.clone().unlocked().password;
                next.new_password = !next.password.is_empty();
            }
            None => {}
        }
        Ok(next)
    }

    /// The settings with the password read from the keychain, if it is kept there and has
    /// not been read yet. This is the only place the keychain is read.
    pub fn unlocked(mut self) -> Self {
        if self.password.is_empty() && self.password_saved {
            self.password = keychain::get(&self.account()).unwrap_or_default();
        }
        self
    }

    /// Reads the file, without the keychain. A password still in the file is moved to the
    /// keychain and taken out of the file. `VIEWER_MOCK=1` opens the demo whatever was saved,
    /// without changing the file.
    pub fn load(path: &Path) -> Self {
        let file = fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Self>(&text).ok());
        let mut settings = match file {
            Some(mut saved) => {
                if !saved.password.is_empty() {
                    saved.new_password = true;
                    let _ = saved.save(path);
                } else if !saved.password_saved {
                    saved.password = Self::default().password;
                }
                saved
            }
            None => Self::default(),
        };
        if std::env::var("VIEWER_MOCK").is_ok_and(|v| v == "1") {
            settings.demo = true;
        }
        settings
    }

    /// Saves a newly typed password to the keychain, and the rest to the file.
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        if self.new_password && !self.password.is_empty() {
            keychain::set(&self.account(), &self.password)?;
            self.password_saved = true;
            self.new_password = false;
        }
        self.write(path)
    }

    fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| format!("설정을 저장하지 못했습니다: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn the_file_never_holds_the_password() {
        let settings = Settings {
            password: "secret".into(),
            ..Settings::default()
        };
        let text = serde_json::to_string(&settings).unwrap();
        assert!(!text.contains("secret"));
        // An old file with a password still reads, so the password can be moved.
        let old: Settings =
            serde_json::from_str(r#"{"neo4j_url":"http://x","password":"old"}"#).unwrap();
        assert_eq!(old.password, "old");
    }

    // Saving the form without a new password, for the same account, leaves the keychain alone:
    // the password stays unread until Neo4j is reached.
    #[test]
    fn the_same_account_keeps_the_password_unread() {
        let saved = Settings {
            password: String::new(),
            password_saved: true,
            ..Settings::default()
        };
        let next = saved
            .apply(super::Change {
                neo4j_url: saved.neo4j_url.clone(),
                database: saved.database.clone(),
                user: saved.user.clone(),
                password: None,
                demo: true,
            })
            .unwrap();
        assert!(next.password.is_empty());
        assert!(next.password_saved);
        assert!(!next.new_password);
    }
}
