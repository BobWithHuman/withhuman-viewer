//! The developer's hooks: the engine's Claude Code setup (`client/`, from withhuman-core's
//! `deploy/client`), written to the app's settings folder and run with Python, for one folder or
//! every folder of the user. The setup itself writes the hooks, the device's client ID and its
//! registration with the engine; the app only runs it and reads what it printed.

use std::path::{Path, PathBuf};
use std::process::Command;

const CONFIGURE: &str = include_str!("../client/configure.py");
const HOOK: &str = include_str!("../client/hooks/pre-tool-use.py");

/// Where the hooks go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// One project folder (`.claude/settings.json` in it).
    Folder,
    /// Every folder of this user (`~/.claude/settings.json`).
    Everywhere,
    /// Every user and folder of the device, in Claude Code's managed settings, with no other
    /// hooks beside them (`--managed-hooks-only`); installed with an administrator's password.
    Device,
}

/// What a setup ended with: the device's client ID, when it printed one.
#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    pub client_id: Option<String>,
}

/// A program to run, with no console window flashing up for it on Windows.
fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// The places Python is looked for after `PATH`: an app opened from the Finder gets a short one;
/// on Windows, where python.org's installer puts it, for the user and for everyone.
fn python_dirs() -> Vec<PathBuf> {
    if !cfg!(windows) {
        return [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/Library/Frameworks/Python.framework/Versions/Current/bin",
            "/usr/bin",
        ]
        .map(PathBuf::from)
        .to_vec();
    }
    let bases = [
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Programs/Python")),
        std::env::var_os("ProgramFiles").map(PathBuf::from),
    ];
    let mut dirs: Vec<PathBuf> = bases
        .into_iter()
        .flatten()
        .filter_map(|base| std::fs::read_dir(base).ok())
        .flat_map(|entries| entries.flatten().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("Python3"))
        })
        .collect();
    // The newest first: Python312 before Python39.
    dirs.sort_by_key(|p| {
        let n = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        std::cmp::Reverse(n[6..].parse::<u32>().unwrap_or(0))
    });
    dirs
}

/// Whether `candidate` is a Python 3.8 or later. Windows' stand-in that offers the Store's
/// Python fails this, as it runs nothing.
fn usable(candidate: &Path) -> bool {
    candidate.is_file()
        && command(candidate)
            .args(["-c", "import sys; sys.exit(sys.version_info < (3, 8))"])
            .output()
            .is_ok_and(|o| o.status.success())
}

/// A Python 3.8 or later the setup can run with: on `PATH` or where installers put it, else on
/// Windows the one the Python launcher (`py -3`) names.
fn python() -> Result<PathBuf, String> {
    let names: &[&str] = if cfg!(windows) {
        &["python.exe", "python3.exe"]
    } else {
        &["python3"]
    };
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path).chain(python_dirs()) {
        if let Some(found) = names.iter().map(|n| dir.join(n)).find(|c| usable(c)) {
            return Ok(found);
        }
    }
    if cfg!(windows) {
        let named = command("py")
            .args(["-3", "-c", "import sys; print(sys.executable)"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
        if let Some(found) = named.filter(|p| usable(p)) {
            return Ok(found);
        }
    }
    Err("Python 3.8 이상을 찾지 못했습니다.".into())
}

/// The setup's two files, written where the app keeps its own; the hook beside it as the setup
/// expects (`hooks/pre-tool-use.py`).
fn files() -> Result<PathBuf, String> {
    let dir = crate::api::config_dir().join("client");
    let hooks = dir.join("hooks");
    std::fs::create_dir_all(&hooks).map_err(|e| format!("설치 파일을 쓰지 못했습니다: {e}"))?;
    std::fs::write(dir.join("configure.py"), CONFIGURE)
        .and_then(|()| std::fs::write(hooks.join("pre-tool-use.py"), HOOK))
        .map_err(|e| format!("설치 파일을 쓰지 못했습니다: {e}"))?;
    Ok(dir)
}

/// The setup's arguments: where, against which engine, with the user's key when given; or
/// taking a setup out.
fn arguments(
    scope: Scope,
    folder: Option<&Path>,
    server: &str,
    key: Option<&str>,
    uninstall: bool,
) -> Result<Vec<String>, String> {
    let mut args = vec![
        "--scope".to_owned(),
        match scope {
            Scope::Folder => "project",
            Scope::Everywhere => "user",
            Scope::Device => "managed",
        }
        .to_owned(),
    ];
    if scope == Scope::Device && !uninstall {
        args.push("--managed-hooks-only".into());
    }
    if scope == Scope::Folder {
        let folder = folder.ok_or("폴더를 골라 주세요.")?;
        args.push("--project".into());
        args.push(folder.display().to_string());
    }
    if uninstall {
        args.push("--uninstall".into());
        return Ok(args);
    }
    let server = server.trim();
    if server.is_empty() {
        return Err("엔진 주소를 넣어 주세요.".into());
    }
    args.push("--server".into());
    args.push(server.to_owned());
    // A device's setup is for every user: each keeps their own key.
    if let Some(key) = key
        .map(str::trim)
        .filter(|k| !k.is_empty() && scope != Scope::Device)
    {
        args.push("--key".into());
        args.push(key.to_owned());
    }
    Ok(args)
}

/// The client ID a setup printed (`WITHHUMAN_CLIENT_ID=<team>/<uuid>`).
fn client_id(output: &str) -> Option<String> {
    output
        .lines()
        .find_map(|l| l.trim().strip_prefix("WITHHUMAN_CLIENT_ID="))
        .map(str::to_owned)
}

/// Installs the hooks, or takes them out (`uninstall`). Blocks while the setup runs, which
/// registers the device with the engine too.
pub fn run(
    scope: Scope,
    folder: Option<&Path>,
    server: &str,
    key: Option<&str>,
    uninstall: bool,
) -> Result<Done, String> {
    let args = arguments(scope, folder, server, key, uninstall)?;
    let out = setup(&args, scope == Scope::Device)?;
    Ok(Done {
        client_id: client_id(&out),
    })
}

/// Takes every WithHuman setup on this machine out at once (`--reset`): the device's, the
/// user's, and those of `folders`, however they were made. The device's client ID is kept, so
/// a setup made after is the same device. Asks for an administrator's password when a
/// device-wide setup is there to take out.
pub fn reset(folders: &[String]) -> Result<(), String> {
    let mut args = vec!["--reset".to_owned()];
    args.extend(folders.iter().cloned());
    let device =
        managed_file().is_file() || device_folder().join("hooks/pre-tool-use.py").is_file();
    setup(&args, device).map(|_| ())
}

/// Runs the setup with `args`, as an administrator when `admin`; what it printed, or the last
/// line it said when it failed.
fn setup(args: &[String], admin: bool) -> Result<String, String> {
    let python = python()?;
    let dir = files()?;
    let ran = if admin {
        as_administrator(&python, &dir.join("configure.py"), args)?
    } else {
        command(python)
            .arg(dir.join("configure.py"))
            .args(args)
            .env("PYTHONIOENCODING", "utf-8")
            .output()
            .map(Ran::from)
            .map_err(|e| format!("설치를 실행하지 못했습니다: {e}"))?
    };
    // AppleScript hands back a script's lines ended by `\r`; Windows ends them with `\r\n`.
    let out = ran.out.replace("\r\n", "\n").replace('\r', "\n");
    if !ran.ok {
        let err = ran.err;
        let said = err
            .lines()
            .chain(out.lines())
            .map(str::trim)
            .rfind(|l| !l.is_empty())
            .unwrap_or("설치가 실패했습니다.");
        return Err(said.to_owned());
    }
    Ok(out)
}

/// What a run of the setup ended with: whether it succeeded, and what it printed and said.
struct Ran {
    ok: bool,
    out: String,
    err: String,
}

impl From<std::process::Output> for Ran {
    fn from(o: std::process::Output) -> Self {
        Ran {
            ok: o.status.success(),
            out: String::from_utf8_lossy(&o.stdout).into_owned(),
            err: String::from_utf8_lossy(&o.stderr).into_owned(),
        }
    }
}

/// A shell word: `value` in single quotes, its own single quotes kept.
#[cfg_attr(windows, allow(dead_code))]
fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Runs the setup as root, asking for an administrator's password as macOS does (`osascript`
/// … `with administrator privileges`). The user who asked is passed on (`SUDO_USER`), so the
/// device keeps the client ID their own setup gave it. The command goes in a script file, so
/// nothing in it needs AppleScript's quoting.
#[cfg(not(windows))]
fn as_administrator(python: &Path, configure: &Path, args: &[String]) -> Result<Ran, String> {
    if !cfg!(target_os = "macos") {
        return Err("기기 전체 설치는 macOS와 Windows에서만 앱으로 할 수 있습니다.".into());
    }
    let user = std::env::var("USER").unwrap_or_default();
    let mut line = format!(
        "SUDO_USER={} {} {}",
        quoted(&user),
        quoted(&python.display().to_string()),
        quoted(&configure.display().to_string())
    );
    for arg in args {
        line.push(' ');
        line.push_str(&quoted(arg));
    }
    let script = configure.with_file_name("as-administrator.sh");
    std::fs::write(&script, format!("#!/bin/sh\n{line}\n"))
        .map_err(|e| format!("설치 파일을 쓰지 못했습니다: {e}"))?;
    let apple = format!(
        "do shell script \"/bin/sh \" & quoted form of \"{}\" with administrator privileges",
        script.display()
    );
    let output = Command::new("osascript")
        .args(["-e", &apple])
        .output()
        .map_err(|e| format!("관리자 권한으로 실행하지 못했습니다: {e}"))?;
    let _ = std::fs::remove_file(&script);
    // Cancelling the password prompt is error -128.
    if String::from_utf8_lossy(&output.stderr).contains("-128") {
        return Err("관리자 암호 입력을 취소했습니다.".into());
    }
    Ok(output.into())
}

/// A word of a Windows batch file: in double quotes, a `%` doubled so it is not read as a
/// variable. A path or URL holds no double quote.
#[cfg_attr(not(windows), allow(dead_code))]
fn batch_word(value: &str) -> String {
    format!("\"{}\"", value.replace('%', "%%"))
}

/// The batch file that runs the setup with `args`, its output and errors to `out` and `err`.
#[cfg_attr(not(windows), allow(dead_code))]
fn batch(python: &Path, configure: &Path, args: &[String], out: &Path, err: &Path) -> String {
    let mut line = format!(
        "{} {}",
        batch_word(&python.display().to_string()),
        batch_word(&configure.display().to_string())
    );
    for arg in args {
        line.push(' ');
        line.push_str(&batch_word(arg));
    }
    format!(
        "@echo off\r\nset PYTHONIOENCODING=utf-8\r\n{line} > {} 2> {}\r\nexit /b %ERRORLEVEL%\r\n",
        batch_word(&out.display().to_string()),
        batch_word(&err.display().to_string())
    )
}

/// Runs the setup as an administrator on Windows: a batch file started with `RunAs`, which
/// shows the UAC prompt. An elevated process cannot write to this one's pipes, so the batch
/// file writes what the setup printed and said to files beside it, read back here.
#[cfg(windows)]
fn as_administrator(python: &Path, configure: &Path, args: &[String]) -> Result<Ran, String> {
    let script = configure.with_file_name("as-administrator.cmd");
    let out = configure.with_file_name("as-administrator.out");
    let err = configure.with_file_name("as-administrator.err");
    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_file(&err);
    std::fs::write(&script, batch(python, configure, args, &out, &err))
        .map_err(|e| format!("설치 파일을 쓰지 못했습니다: {e}"))?;
    // Refusing the UAC prompt throws; that is told apart by exit code 1223, Windows' own for a
    // cancelled operation.
    let elevate = "try { $p = Start-Process -FilePath $env:WH_SCRIPT -Verb RunAs -Wait -PassThru \
                   -WindowStyle Hidden -ErrorAction Stop; exit $p.ExitCode } catch { exit 1223 }";
    let status = command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", elevate])
        .env("WH_SCRIPT", &script)
        .status()
        .map_err(|e| format!("관리자 권한으로 실행하지 못했습니다: {e}"))?;
    let _ = std::fs::remove_file(&script);
    let read = |path: &Path| {
        let text = std::fs::read(path)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .ok();
        let _ = std::fs::remove_file(path);
        text
    };
    let (printed, said) = (read(&out), read(&err));
    if status.code() == Some(1223) && printed.is_none() {
        return Err("관리자 권한 확인을 취소했습니다.".into());
    }
    Ok(Ran {
        ok: status.success(),
        out: printed.unwrap_or_default(),
        err: said.unwrap_or_default(),
    })
}

/// This device's client ID: the one a device-wide setup keeps, else the user's own setup's.
pub fn device_id() -> Option<String> {
    let mut files = vec![device_folder().join("client-id")];
    if let Some(home) = dirs::home_dir() {
        files.push(home.join(".config/withhuman/client-id"));
    }
    files.into_iter().find_map(|path| {
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    })
}

/// Where a device-wide setup keeps the hook and the device's client ID, by system.
fn device_folder() -> PathBuf {
    PathBuf::from(if cfg!(target_os = "macos") {
        "/Library/Application Support/WithHuman"
    } else if cfg!(windows) {
        "C:\\Program Files\\WithHuman"
    } else {
        "/opt/withhuman"
    })
}

/// This machine's name as its owner knows it (macOS's computer name, else the host name) and
/// its system with the version.
pub fn this_machine() -> (String, String) {
    let said = |program: &str, args: &[&str]| {
        command(program)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    let name = if cfg!(target_os = "macos") {
        said("scutil", &["--get", "ComputerName"])
    } else {
        std::env::var("COMPUTERNAME").ok().filter(|n| !n.is_empty())
    }
    .or_else(|| said("hostname", &[]))
    .unwrap_or_else(|| "이 기기".into());
    let os = match std::env::consts::OS {
        "macos" => format!(
            "macOS {}",
            said("sw_vers", &["-productVersion"]).unwrap_or_default()
        ),
        // `Microsoft Windows 11 Pro`, its maker left off.
        "windows" => said(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-CimInstance Win32_OperatingSystem).Caption",
            ],
        )
        .map(|c| c.trim_start_matches("Microsoft ").to_owned())
        .unwrap_or_else(|| "Windows".into()),
        other => other.to_owned(),
    };
    (name, os.trim().to_owned())
}

/// Whether the engine at `server` answers (`/healthz`), and how fast, in milliseconds.
pub async fn check_engine(server: &str) -> Result<u128, String> {
    let url = format!("{}/healthz", server.trim().trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    let started = std::time::Instant::now();
    let response = client.get(&url).send().await.map_err(|e| {
        if e.is_timeout() {
            "응답 없음".to_owned()
        } else {
            "연결 안 됨".to_owned()
        }
    })?;
    if response.status().is_success() {
        Ok(started.elapsed().as_millis())
    } else {
        Err(format!("HTTP {}", response.status().as_u16()))
    }
}

/// The sessions of this machine alone: those its hooks sent with its client ID. None without a
/// setup on this machine.
pub fn mine(sessions: Vec<crate::model::Session>) -> Vec<crate::model::Session> {
    let Some(id) = device_id() else {
        return Vec::new();
    };
    sessions
        .into_iter()
        .filter(|s| s.client.as_deref() == Some(id.as_str()))
        .collect()
}

/// Where a WithHuman setup was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Where {
    /// Every folder of this user.
    Everywhere,
    /// One project folder.
    Folder(PathBuf),
    /// Every user and folder of the device, set by an administrator.
    Device,
}

/// A WithHuman setup found on this machine: where, its settings file, and the engine its hooks
/// report to.
#[derive(Clone, Debug, PartialEq)]
pub struct Install {
    pub at: Where,
    pub file: PathBuf,
    pub engine: Option<String>,
    /// When its settings file was last written, epoch milliseconds.
    pub at_ms: Option<i64>,
}

/// The engine the WithHuman hooks in Claude Code settings report to; none when the settings
/// hold no WithHuman hook. An HTTP hook names it in its URL, the PreToolUse command hook as its
/// last argument.
fn engine_of(settings: &serde_json::Value) -> Option<Option<String>> {
    let mut found = None;
    for groups in settings["hooks"].as_object()?.values() {
        for group in groups.as_array().into_iter().flatten() {
            for hook in group["hooks"].as_array().into_iter().flatten() {
                let url = hook["url"].as_str().filter(|u| u.contains("/v1/hooks/"));
                let command = hook["command"]
                    .as_str()
                    .filter(|c| c.contains("pre-tool-use.py"))
                    .and_then(|c| c.split_whitespace().last());
                if let Some(target) = url.or(command) {
                    let engine = target.split("/v1/hooks/").next().map(str::to_owned);
                    found = Some(engine.filter(|e| e.starts_with("http")));
                }
            }
        }
    }
    found
}

/// The setup in the settings file at `file`, if it holds WithHuman's hooks.
fn found(at: Where, file: PathBuf) -> Option<Install> {
    let text = std::fs::read_to_string(&file).ok()?;
    let settings: serde_json::Value = serde_json::from_str(&text).ok()?;
    let engine = engine_of(&settings)?;
    let at_ms = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);
    Some(Install {
        at,
        file,
        engine,
        at_ms,
    })
}

/// Claude Code's managed settings file WithHuman's device-wide setup writes, by system.
fn managed_file() -> PathBuf {
    let folder = if cfg!(target_os = "macos") {
        "/Library/Application Support/ClaudeCode"
    } else if cfg!(windows) {
        "C:\\Program Files\\ClaudeCode"
    } else {
        "/etc/claude-code"
    };
    Path::new(folder).join("managed-settings.d/50-withhuman.json")
}

/// The WithHuman setups on this machine: the device's, the user's, and those in `folders` (the
/// ones the app installed); the folders whose setup is gone are left out.
pub fn installed(folders: &[String]) -> Vec<Install> {
    let mut out = Vec::new();
    out.extend(found(Where::Device, managed_file()));
    if let Some(home) = dirs::home_dir() {
        out.extend(found(Where::Everywhere, home.join(".claude/settings.json")));
    }
    for folder in folders {
        let folder = PathBuf::from(folder);
        let file = folder.join(".claude/settings.json");
        out.extend(found(Where::Folder(folder), file));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_engine_withhumans_hooks_report_to() {
        let settings = serde_json::json!({"hooks": {
            "PreToolUse": [{"hooks": [{"type": "command", "command": "\"/usr/bin/python3\" \"/h/pre-tool-use.py\" https://e.example/v1/hooks/PreToolUse"}]}],
            "Stop": [{"hooks": [{"type": "command", "command": "echo mine"}]}],
        }});
        assert_eq!(engine_of(&settings), Some(Some("https://e.example".into())));
        let http = serde_json::json!({"hooks": {"SessionStart": [{"hooks": [{"type": "http", "url": "http://127.0.0.1:3000/v1/hooks/SessionStart"}]}]}});
        assert_eq!(engine_of(&http), Some(Some("http://127.0.0.1:3000".into())));
        let mine = serde_json::json!({"model": "opus", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "echo"}]}]}});
        assert_eq!(engine_of(&mine), None);
        assert_eq!(engine_of(&serde_json::json!({})), None);
    }

    #[test]
    fn a_folder_setup_names_its_folder_and_a_global_one_none() {
        let folder = Path::new("/work/app");
        assert_eq!(
            arguments(
                Scope::Folder,
                Some(folder),
                " http://e:3000 ",
                Some(" "),
                false
            )
            .unwrap(),
            [
                "--scope",
                "project",
                "--project",
                "/work/app",
                "--server",
                "http://e:3000"
            ]
        );
        assert_eq!(
            arguments(Scope::Everywhere, None, "http://e", Some("whk_x"), false).unwrap(),
            ["--scope", "user", "--server", "http://e", "--key", "whk_x"]
        );
        assert_eq!(
            arguments(Scope::Everywhere, None, "", None, true).unwrap(),
            ["--scope", "user", "--uninstall"]
        );
        // The device's setup runs no other hooks beside it, and takes no one's key.
        assert_eq!(
            arguments(Scope::Device, None, "http://e", Some("whk_x"), false).unwrap(),
            [
                "--scope",
                "managed",
                "--managed-hooks-only",
                "--server",
                "http://e"
            ]
        );
        assert_eq!(
            arguments(Scope::Device, None, "", None, true).unwrap(),
            ["--scope", "managed", "--uninstall"]
        );
        assert!(arguments(Scope::Folder, None, "http://e", None, false).is_err());
        assert!(arguments(Scope::Everywhere, None, " ", None, false).is_err());
    }

    #[test]
    fn writes_the_batch_file_that_runs_the_setup_as_an_administrator() {
        let script = batch(
            Path::new("C:\\Python312\\python.exe"),
            Path::new("C:\\Users\\a b\\client\\configure.py"),
            &["--reset".into(), "C:\\work\\100% app".into()],
            Path::new("C:\\t\\o"),
            Path::new("C:\\t\\e"),
        );
        assert!(script.contains(
            "\"C:\\Python312\\python.exe\" \"C:\\Users\\a b\\client\\configure.py\" \"--reset\" \"C:\\work\\100%% app\" > \"C:\\t\\o\" 2> \"C:\\t\\e\"\r\n"
        ));
        assert!(script.starts_with("@echo off\r\n"));
        assert!(script.ends_with("exit /b %ERRORLEVEL%\r\n"));
    }

    #[test]
    fn quotes_a_shell_word_with_its_own_quotes() {
        assert_eq!(quoted("a b"), "'a b'");
        assert_eq!(quoted("it's"), "'it'\\''s'");
    }

    #[test]
    fn reads_the_client_id_the_setup_printed() {
        assert_eq!(
            client_id("a\nWITHHUMAN_CLIENT_ID=default/1234\nb").as_deref(),
            Some("default/1234")
        );
        assert_eq!(client_id("nothing"), None);
    }
}
