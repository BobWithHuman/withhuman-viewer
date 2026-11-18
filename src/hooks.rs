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

/// The places Python is looked for after `PATH`: an app opened from the Finder gets a short one.
const PYTHON_DIRS: [&str; 4] = [
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/Library/Frameworks/Python.framework/Versions/Current/bin",
    "/usr/bin",
];

/// A Python 3.8 or later the setup can run with.
fn python() -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let dirs = std::env::split_paths(&path).chain(PYTHON_DIRS.iter().map(PathBuf::from));
    for dir in dirs {
        let candidate = dir.join("python3");
        if !candidate.is_file() {
            continue;
        }
        let ok = Command::new(&candidate)
            .args(["-c", "import sys; sys.exit(sys.version_info < (3, 8))"])
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            return Ok(candidate);
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
    let python = python()?;
    let dir = files()?;
    let output = if scope == Scope::Device {
        as_administrator(&python, &dir.join("configure.py"), &args)?
    } else {
        Command::new(python)
            .arg(dir.join("configure.py"))
            .args(&args)
            .output()
            .map_err(|e| format!("설치를 실행하지 못했습니다: {e}"))?
    };
    // AppleScript hands back a script's lines ended by `\r`.
    let out = String::from_utf8_lossy(&output.stdout).replace('\r', "\n");
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let said = err
            .lines()
            .chain(out.lines())
            .map(str::trim)
            .rfind(|l| !l.is_empty())
            .unwrap_or("설치가 실패했습니다.");
        return Err(said.to_owned());
    }
    Ok(Done {
        client_id: client_id(&out),
    })
}

/// A shell word: `value` in single quotes, its own single quotes kept.
fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Runs the setup as root, asking for an administrator's password as macOS does (`osascript`
/// … `with administrator privileges`). The user who asked is passed on (`SUDO_USER`), so the
/// device keeps the client ID their own setup gave it. The command goes in a script file, so
/// nothing in it needs AppleScript's quoting.
fn as_administrator(
    python: &Path,
    configure: &Path,
    args: &[String],
) -> Result<std::process::Output, String> {
    if !cfg!(target_os = "macos") {
        return Err("기기 전체 설치는 macOS에서만 앱으로 할 수 있습니다.".into());
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
    Ok(output)
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
    Some(Install { at, file, engine })
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
