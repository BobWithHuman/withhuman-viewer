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
        }
        .to_owned(),
    ];
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
    if let Some(key) = key.map(str::trim).filter(|k| !k.is_empty()) {
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
    let output = Command::new(python)
        .arg(dir.join("configure.py"))
        .args(&args)
        .output()
        .map_err(|e| format!("설치를 실행하지 못했습니다: {e}"))?;
    let out = String::from_utf8_lossy(&output.stdout);
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

/// This device's client ID, from an earlier setup.
pub fn device_id() -> Option<String> {
    let path = dirs::home_dir()?.join(".config/withhuman/client-id");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(arguments(Scope::Folder, None, "http://e", None, false).is_err());
        assert!(arguments(Scope::Everywhere, None, " ", None, false).is_err());
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
