//! The personal app's own engine on this machine, with no Docker: withhuman-core's
//! `withhuman-local`, shipped beside the app's binary, which runs Decision, Generator and
//! Collector in one process on a PostgreSQL it installs in the engine's folder. What it runs as
//! (records but stops nothing, no Redis, no gateway, no key) is the engine's `personal` profile;
//! the app only starts it and hands it its secrets. The app's binary does that
//! (`withhuman-viewer --engine`), started by launchd at login on a Mac so the record goes on with
//! the app closed; elsewhere the app starts it when it opens. The Jev key and the passwords are
//! kept in the keychain, the Jev model in `engine.json`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// launchd's name for the engine.
#[cfg(target_os = "macos")]
const LABEL: &str = "dev.withhuman.engine";
/// The engine's binary, beside the app's.
const ENGINE: &str = "withhuman-local";
/// The personal profile's ports (withhuman-core's `withhuman_common::profile::personal`).
const PG_PORT: u16 = 47432;
const COLLECTOR_PORT: u16 = 47380;
const GENERATOR_PORT: u16 = 47381;
const DECISION_PORT: u16 = 47382;
const DATABASE: &str = "withhuman";
/// The read-only login the Generator creates for the viewer, as the engine's Compose file has it.
pub const VIEWER_USER: &str = "withhuman_viewer";
/// The engine's own default (withhuman-core's `TraceSettings`).
pub const DEFAULT_MODEL: &str = "jev-1.14.0";
/// The keychain accounts of the engine's two passwords.
const PG_ACCOUNT: &str = "engine-postgres";
const VIEWER_ACCOUNT: &str = "engine-viewer";

/// Where the hooks send their events: the Collector.
pub fn collector_url() -> String {
    format!("http://127.0.0.1:{COLLECTOR_PORT}")
}

/// The store the app reads the record from.
pub fn store_url() -> String {
    format!("postgres://127.0.0.1:{PG_PORT}/{DATABASE}")
}

/// The engine's folder: PostgreSQL, its data, the logs and `engine.json`.
pub fn dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("WithHuman")
        .join("engine")
}

pub fn logs() -> PathBuf {
    dir().join("logs")
}

/// What the engine runs with besides its secrets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The Jev model every question is asked of (`JEV_MODEL`).
    pub model: String,
    /// Whether Calls are sent to Jev to be judged, when there is a key. Off, or without a key,
    /// nothing leaves the machine: the engine judges by its rules alone.
    pub jev: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.into(),
            jev: true,
        }
    }
}

impl Config {
    fn path() -> PathBuf {
        dir().join("engine.json")
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(Self::path(), text).map_err(|e| format!("엔진 설정을 저장하지 못했습니다: {e}"))
    }
}

/// The engine's PostgreSQL password and the viewer's, made the first time and kept in the
/// keychain after.
pub fn passwords() -> Result<(String, String), String> {
    let get = |account: &str| -> Result<String, String> {
        if let Some(kept) = crate::settings::secret(account).filter(|p| !p.is_empty()) {
            return Ok(kept);
        }
        let made = random_secret()?;
        crate::settings::keep_secret(account, &made)?;
        Ok(made)
    };
    Ok((get(PG_ACCOUNT)?, get(VIEWER_ACCOUNT)?))
}

/// 24 random bytes in hex: safe in a URL without escaping.
fn random_secret() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    #[cfg(unix)]
    {
        use std::io::Read;
        fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .map_err(|e| format!("비밀번호를 만들지 못했습니다: {e}"))?;
    }
    // Each RandomState is seeded from the system's randomness.
    #[cfg(not(unix))]
    for chunk in bytes.chunks_mut(8) {
        use std::hash::{BuildHasher, Hasher};
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_usize(chunk.as_ptr() as usize);
        chunk.copy_from_slice(&h.finish().to_le_bytes()[..chunk.len()]);
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// The engine's parts, in the order they start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Store,
    Decision,
    Generator,
    Collector,
}

impl Part {
    pub const ALL: [Part; 4] = [
        Part::Store,
        Part::Decision,
        Part::Generator,
        Part::Collector,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Part::Store => "PostgreSQL",
            Part::Decision => "Decision",
            Part::Generator => "Generator",
            Part::Collector => "Collector",
        }
    }

    fn port(self) -> u16 {
        match self {
            Part::Store => PG_PORT,
            Part::Decision => DECISION_PORT,
            Part::Generator => GENERATOR_PORT,
            Part::Collector => COLLECTOR_PORT,
        }
    }
}

/// Whether each part answers: PostgreSQL takes a connection, the services their `/healthz`.
pub async fn health() -> Vec<(Part, bool)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("HTTP client");
    let mut out = Vec::new();
    for part in Part::ALL {
        out.push((part, answers(&client, part).await));
    }
    out
}

/// Whether the Collector, where the hooks send, takes a connection: cheap enough to ask every few
/// seconds.
pub async fn collector_answers() -> bool {
    tokio::task::spawn_blocking(|| {
        std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], COLLECTOR_PORT)),
            Duration::from_millis(500),
        )
        .is_ok()
    })
    .await
    .unwrap_or(false)
}

async fn answers(client: &reqwest::Client, part: Part) -> bool {
    if part == Part::Store {
        let port = part.port();
        return tokio::task::spawn_blocking(move || {
            std::net::TcpStream::connect_timeout(
                &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
                Duration::from_millis(500),
            )
            .is_ok()
        })
        .await
        .unwrap_or(false);
    }
    let url = format!("http://127.0.0.1:{}/healthz", part.port());
    client
        .get(url)
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

/// The folder `withhuman-local` is in: `WITHHUMAN_ENGINE_BIN`, else beside the app's own (the
/// packaged app), else a withhuman-core checkout's release build.
fn binaries() -> Result<PathBuf, String> {
    let has_all = |d: &Path| d.join(exe_name(ENGINE)).is_file();
    let mut candidates = Vec::new();
    if let Some(d) = std::env::var_os("WITHHUMAN_ENGINE_BIN") {
        candidates.push(PathBuf::from(d));
    }
    if let Some(d) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        candidates.push(d);
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("withhuman-core/target/release"));
    }
    candidates
        .into_iter()
        .find(|d| has_all(d))
        .ok_or_else(|| "엔진 실행 파일(withhuman-local)을 찾지 못했습니다.".into())
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// What `withhuman-local` is handed: its folder and secrets, and the Jev key and model when
/// Calls go to Jev. Without a key the engine judges by its rules alone. The rest is its
/// `personal` profile's.
fn environment(
    jev_key: Option<&str>,
    pg_password: &str,
    viewer: &str,
) -> Vec<(&'static str, String)> {
    let mut env = vec![
        ("RUST_LOG", "info".to_owned()),
        ("WITHHUMAN_HOME", dir().display().to_string()),
        ("WITHHUMAN_PG_PASSWORD", pg_password.to_owned()),
        ("WITHHUMAN_VIEWER_PASSWORD", viewer.to_owned()),
    ];
    if let Some(key) = jev_key {
        env.push(("JEV_API_KEY", key.to_owned()));
        env.push(("JEV_MODEL", Config::load().model));
        if let Ok(jev) = std::env::var("JEV_URL") {
            env.push(("JEV_URL", jev));
        }
    }
    env
}

/// The Jev key Calls are sent with: `JEV_API_KEY` (a run from a terminal), else the keychain's,
/// and none while sending is off.
pub fn jev_in_use() -> Option<String> {
    if !Config::load().jev {
        return None;
    }
    std::env::var("JEV_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .or_else(crate::settings::jev_key)
}

/// `withhuman-viewer --engine`: becomes `withhuman-local` with the keychain's secrets, so
/// launchd's stop reaches the engine itself. Exits 1 when it cannot start.
pub fn serve() -> i32 {
    let jev = jev_in_use();
    // The engine's own `withhuman-local` reads JEV_API_KEY: one left in this environment while
    // sending is off must not reach it.
    let started = passwords().and_then(|(pg_password, viewer)| {
        let program = binaries()?.join(exe_name(ENGINE));
        fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
        let mut command = std::process::Command::new(program);
        command
            .env_remove("JEV_API_KEY")
            .envs(environment(jev.as_deref(), &pg_password, &viewer))
            .stdin(Stdio::null());
        run(command)
    });
    match started {
        Ok(code) => code,
        Err(why) => {
            eprintln!("withhuman engine: {why}");
            1
        }
    }
}

/// On Unix the engine takes this process's place; elsewhere it runs as a child, waited for.
#[cfg(unix)]
fn run(mut command: std::process::Command) -> Result<i32, String> {
    use std::os::unix::process::CommandExt;
    // Returns only when the engine could not be started.
    Err(format!("엔진을 시작하지 못했습니다: {}", command.exec()))
}

#[cfg(not(unix))]
fn run(mut command: std::process::Command) -> Result<i32, String> {
    let status = command
        .status()
        .map_err(|e| format!("엔진을 시작하지 못했습니다: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

/// Starts the engine if it is not running: on a Mac, launchd's agent for it, written again when
/// the app has moved; elsewhere a process of its own.
pub fn ensure_started() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        let path = agent_file()?;
        // launchd hands the engine no shell's variables: where its binaries are is written in.
        let plist = agent_plist(&exe, &binaries()?);
        let changed = fs::read_to_string(&path).ok().as_deref() != Some(plist.as_str());
        if changed {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::create_dir_all(logs()).map_err(|e| e.to_string())?;
            fs::write(&path, plist).map_err(|e| format!("launchd 파일을 쓰지 못했습니다: {e}"))?;
            let _ = launchctl(&["bootout", &service()?]);
        }
        if changed || !launchctl(&["print", &service()?]).is_ok_and(|ok| ok) {
            let ok = launchctl(&["bootstrap", &domain()?, &path.display().to_string()])?;
            if !ok && !launchctl(&["print", &service()?]).is_ok_and(|ok| ok) {
                return Err("launchd가 엔진을 받지 않았습니다.".into());
            }
        }
        // Loaded but exited: start it now; and an engine an earlier version of the app started
        // (the app was updated around it) again, so the one inside this app runs.
        let updated = started_by() != crate::updater::VERSION;
        let service = service()?;
        if updated {
            let _ = launchctl(&["kickstart", "-k", &service]);
        } else {
            let _ = launchctl(&["kickstart", &service]);
        }
        if updated {
            let _ = fs::write(dir().join("started-by"), crate::updater::VERSION);
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        fs::create_dir_all(logs()).map_err(|e| e.to_string())?;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(logs().join("engine.log"))
            .map_err(|e| e.to_string())?;
        std::process::Command::new(exe)
            .arg("--engine")
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("엔진을 시작하지 못했습니다: {e}"))
    }
}

/// The app version that last started the engine.
#[cfg(target_os = "macos")]
fn started_by() -> String {
    fs::read_to_string(dir().join("started-by"))
        .map(|v| v.trim().to_owned())
        .unwrap_or_default()
}

/// Starts the engine again, so a new Jev key or model applies.
pub fn restart() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        ensure_started()?;
        if launchctl(&["kickstart", "-k", &service()?])? {
            Ok(())
        } else {
            Err("엔진을 다시 시작하지 못했습니다.".into())
        }
    }
    #[cfg(not(target_os = "macos"))]
    ensure_started()
}

/// Stops the engine for good and takes it off this machine: launchd's agent for it, and its
/// folder (PostgreSQL, the record, the logs). The secrets in the keychain are the app's to take.
pub fn uninstall() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = launchctl(&["bootout", &service()?]);
        let path = agent_file()?;
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("launchd 파일을 지우지 못했습니다: {e}"))?;
        }
    }
    // Elsewhere the engine is a process of its own, started when the app opened.
    #[cfg(windows)]
    let _ = std::process::Command::new("taskkill")
        .args(["/IM", "withhuman-local.exe", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("pkill")
        .args(["-x", ENGINE])
        .status();
    // A server the engine left running holds the data folder: stopped before it is removed.
    let pg_ctl = dir().join("postgresql");
    if let Ok(versions) = fs::read_dir(&pg_ctl) {
        for version in versions.flatten() {
            let program = version.path().join("bin").join(exe_name("pg_ctl"));
            if program.is_file() {
                let _ = std::process::Command::new(program)
                    .args(["stop", "-m", "fast", "-D"])
                    .arg(dir().join("pg"))
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
    }
    match fs::remove_dir_all(dir()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("엔진 폴더를 지우지 못했습니다: {e}")),
    }
}

/// The keychain accounts the engine keeps.
pub const ACCOUNTS: [&str; 2] = [PG_ACCOUNT, VIEWER_ACCOUNT];

#[cfg(target_os = "macos")]
fn agent_file() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|h| {
            h.join("Library/LaunchAgents")
                .join(format!("{LABEL}.plist"))
        })
        .ok_or_else(|| "홈 폴더를 찾지 못했습니다.".into())
}

#[cfg(target_os = "macos")]
fn domain() -> Result<String, String> {
    use std::os::unix::fs::MetadataExt;
    let home = dirs::home_dir().ok_or("홈 폴더를 찾지 못했습니다.")?;
    let uid = fs::metadata(home).map_err(|e| e.to_string())?.uid();
    Ok(format!("gui/{uid}"))
}

#[cfg(target_os = "macos")]
fn service() -> Result<String, String> {
    Ok(format!("{}/{LABEL}", domain()?))
}

/// Runs `launchctl`; whether it succeeded.
#[cfg(target_os = "macos")]
fn launchctl(args: &[&str]) -> Result<bool, String> {
    std::process::Command::new("/bin/launchctl")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .map_err(|e| format!("launchctl을 실행하지 못했습니다: {e}"))
}

/// launchd's agent: the app's binary with `--engine`, at login and again whenever it fails.
#[cfg(target_os = "macos")]
fn agent_plist(exe: &Path, bin: &Path) -> String {
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let exe = escape(&exe.display().to_string());
    let bin = escape(&bin.display().to_string());
    let log = escape(&logs().join("engine.log").display().to_string());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>--engine</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>WITHHUMAN_ENGINE_BIN</key>
    <string>{bin}</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>ThrottleInterval</key>
  <integer>10</integer>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_is_url_safe_and_new_each_time() {
        let a = random_secret().unwrap();
        let b = random_secret().unwrap();
        assert_eq!(a.len(), 48);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn the_engine_is_handed_its_secrets_and_nothing_of_its_profile() {
        let env = environment(Some("key"), "pg", "v");
        let get = |name: &str| {
            env.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("JEV_API_KEY"), Some("key"));
        assert_eq!(get("WITHHUMAN_PG_PASSWORD"), Some("pg"));
        assert_eq!(get("WITHHUMAN_VIEWER_PASSWORD"), Some("v"));
        assert_eq!(
            get("WITHHUMAN_HOME"),
            Some(dir().display().to_string().as_str())
        );
        // The engine's profile decides these, not the app.
        for decided in [
            "WITHHUMAN_MODE",
            "WITHHUMAN_PROFILE",
            "REDIS_URL",
            "COLLECTOR_ADDR",
        ] {
            assert_eq!(get(decided), None, "{decided}");
        }
        assert_eq!(collector_url(), "http://127.0.0.1:47380");
        assert_eq!(store_url(), "postgres://127.0.0.1:47432/withhuman");
        // Without a key nothing of Jev's is handed: the engine judges by its rules.
        let local = environment(None, "pg", "v");
        assert!(!local.iter().any(|(k, _)| k.starts_with("JEV_")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launchd_runs_the_apps_binary_as_the_engine() {
        let plist = agent_plist(
            Path::new("/Applications/With <Human>.app/Contents/MacOS/x"),
            Path::new("/Applications/With <Human>.app/Contents/MacOS"),
        );
        assert!(plist.contains("<string>--engine</string>"));
        assert!(plist.contains("<key>WITHHUMAN_ENGINE_BIN</key>"));
        assert!(plist.contains("With &lt;Human&gt;.app"));
        assert!(plist.contains("<key>SuccessfulExit</key>\n    <false/>"));
    }
}
