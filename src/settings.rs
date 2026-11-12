//! Where the viewer finds the engine's data (its PostgreSQL, or Neo4j before withhuman-core #118),
//! kept in the app's config folder between launches. The password is kept in the system keychain,
//! never in the file.

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
    /// Empty when the file has none, not the struct default (`NEO4J_PASSWORD` or
    /// `withhuman-e2e`): that would look like an old file's password and be moved to the
    /// keychain on every launch, over the saved one.
    #[serde(default, skip_serializing)]
    pub password: String,
    /// The keychain holds this account's password.
    pub password_saved: bool,
    /// A password typed in the settings, to be put in the keychain on save.
    #[serde(skip)]
    new_password: bool,
    /// Reads the bundled demo graph instead of Neo4j.
    pub demo: bool,
    /// No settings were saved yet: the app greets instead of reporting a failed connection.
    #[serde(skip)]
    pub first_run: bool,
    /// Where the engine runs, as chosen on the first launch.
    pub engine: Engine,
    /// The engine's folder, when the viewer starts it on this Mac.
    pub engine_dir: Option<String>,
    /// A Jev key was taken by Jev and is kept in the keychain.
    pub jev_saved: bool,
    /// The engine's address for the security team's key actions (Collector's /v1/admin, through
    /// the gateway).
    pub engine_api: String,
    /// The keychain holds the engine's admin token for `engine_api`.
    pub admin_token_saved: bool,
}

/// Where the engine runs: started by the viewer on this Mac, or reached on a server.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    #[default]
    Unset,
    Local,
    Server,
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
            first_run: false,
            engine: Engine::Unset,
            engine_dir: None,
            jev_saved: false,
            engine_api: env("WITHHUMAN_ENGINE_API", "http://127.0.0.1:3000"),
            admin_token_saved: false,
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
    pub first_run: bool,
    pub path: String,
    pub engine: Engine,
    pub engine_dir: Option<String>,
    pub jev_saved: bool,
    pub engine_api: String,
    pub has_admin_token: bool,
}

impl Public {
    /// Where the data comes from, without the credentials an address may carry: the engine's
    /// PostgreSQL, Neo4j (engines before withhuman-core #118), or the demo.
    pub fn store(&self) -> String {
        if self.demo {
            return "데모 데이터".into();
        }
        let url = self.neo4j_url.trim();
        match url.split_once("://") {
            Some(("postgres" | "postgresql", _)) => format!("PostgreSQL · {}", self.host()),
            _ => format!("Neo4j · {url}"),
        }
    }

    /// The address without its scheme, credentials and options, for the top bar.
    pub fn host(&self) -> String {
        let url = self.neo4j_url.trim();
        let rest = url.split_once("://").map_or(url, |(_, r)| r);
        let rest = rest.split('?').next().unwrap_or(rest);
        rest.rsplit('@').next().unwrap_or(rest).to_owned()
    }
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

/// The keychain account the Jev API key is kept under.
const JEV_ACCOUNT: &str = "jev-api-key";

/// Keeps a Jev API key Jev took in the keychain.
pub fn save_jev_key(key: &str) -> Result<(), String> {
    keychain::set(JEV_ACCOUNT, key.trim())
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
            // Saved by the person, not the development default the app starts with.
            has_password: self.password_saved,
            demo: self.demo,
            first_run: self.first_run,
            path: path.display().to_string(),
            engine: self.engine,
            engine_dir: self.engine_dir.clone(),
            jev_saved: self.jev_saved,
            engine_api: self.engine_api.clone(),
            has_admin_token: self.admin_token_saved,
        }
    }

    /// The keychain account of the admin token: another engine is another token.
    fn admin_account(&self) -> String {
        format!("admin-token@{}", self.engine_api)
    }

    /// The engine's admin token, read from the keychain only when a key action needs it.
    pub fn admin_token(&self) -> Option<String> {
        if !self.admin_token_saved {
            return None;
        }
        keychain::get(&self.admin_account()).filter(|t| !t.is_empty())
    }

    /// These settings with another engine address for key actions, and a new admin token kept
    /// in the keychain if one is given.
    pub fn with_admin(&self, engine_api: &str, token: Option<&str>) -> Result<Self, String> {
        let url = engine_api.trim().trim_end_matches('/').to_owned();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("엔진 관리 API 주소는 http:// 또는 https://로 시작해야 합니다.".into());
        }
        let mut next = self.clone();
        next.engine_api = url;
        if let Some(token) = token.map(str::trim).filter(|t| !t.is_empty()) {
            keychain::set(&next.admin_account(), token)?;
            next.admin_token_saved = true;
        } else if next.engine_api != self.engine_api {
            next.admin_token_saved = false;
        }
        Ok(next)
    }

    pub fn apply(&self, change: Change) -> Result<Self, String> {
        let url = change.neo4j_url.trim().trim_end_matches('/').to_owned();
        if !["http://", "https://", "postgres://", "postgresql://"].iter().any(|s| url.starts_with(s)) {
            return Err("주소는 postgres://(엔진의 PostgreSQL) 또는 http://(예전 엔진의 Neo4j)로 시작해야 합니다.".into());
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
            first_run: self.first_run,
            engine: self.engine,
            engine_dir: self.engine_dir.clone(),
            jev_saved: self.jev_saved,
            engine_api: self.engine_api.clone(),
            admin_token_saved: self.admin_token_saved,
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

    /// These settings pointed at another database for this run only, as `VIEWER_DB` asks: the
    /// address read as written, its password (if it carries one) kept from the keychain's way,
    /// the demo and the first launch's setup left aside. Nothing of it is saved unless the
    /// settings are saved from the form.
    pub fn pointed_at(mut self, url: &str) -> Self {
        self.neo4j_url = url.trim().to_owned();
        // Without sslmode and sslrootcert, which tokio-postgres doesn't read.
        let without = url.trim().split('?').next().unwrap_or(url);
        if let Ok(config) = <tokio_postgres::Config as std::str::FromStr>::from_str(without)
            && let Some(password) = config.get_password()
        {
            self.password = String::from_utf8_lossy(password).into_owned();
        }
        self.demo = false;
        self.jev_saved = true;
        // Not the engine this Mac may start: the address is what the bar names.
        self.engine = Engine::Server;
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
            None => Self {
                first_run: true,
                ..Self::default()
            },
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
        self.write(path)?;
        self.first_run = false;
        Ok(())
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
    fn the_form_takes_a_postgres_address() {
        let change = |url: &str| super::Change {
            neo4j_url: url.into(),
            database: "withhuman".into(),
            user: "withhuman_viewer".into(),
            password: None,
            demo: false,
        };
        assert!(Settings::default().apply(change("postgres://127.0.0.1:5432/withhuman")).is_ok());
        assert!(Settings::default().apply(change("http://127.0.0.1:7474")).is_ok());
        assert!(Settings::default().apply(change("127.0.0.1:5432")).is_err());
        assert!(Settings::default().with_admin("ftp://x", None).is_err());
        let next = Settings::default().with_admin("http://engine:3000/", None).unwrap();
        assert_eq!((next.engine_api.as_str(), next.admin_token_saved), ("http://engine:3000", false));
    }

    #[test]
    fn the_pages_name_the_store_without_its_credentials() {
        let mut s = Settings::default().pointed_at("postgres://viewer:pw@db.internal:5433/withhuman");
        assert_eq!(s.password, "pw", "taken from the address, so no keychain prompt");
        assert!(!s.demo && s.jev_saved);
        let p = s.public(std::path::Path::new("/tmp/x"));
        assert_eq!(p.store(), "PostgreSQL · db.internal:5433/withhuman");
        assert_eq!(p.host(), "db.internal:5433/withhuman");
        s.neo4j_url = "http://127.0.0.1:7474".into();
        assert_eq!(s.public(std::path::Path::new("/tmp/x")).store(), "Neo4j · http://127.0.0.1:7474");
        // The encryption's options are neither shown nor in the way of the password.
        let s = Settings::default().pointed_at("postgres://viewer:pw@db.example.com/wh?sslmode=verify-full&sslrootcert=/x/ca.pem");
        assert_eq!(s.password, "pw");
        assert_eq!(s.public(std::path::Path::new("/tmp/x")).store(), "PostgreSQL · db.example.com/wh");
    }

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

    // A file the app wrote has no password: it must not read as one to move to the keychain.
    #[test]
    fn a_file_without_a_password_reads_empty() {
        let saved: Settings =
            serde_json::from_str(r#"{"neo4j_url":"http://x","password_saved":true}"#).unwrap();
        assert!(saved.password.is_empty());
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
