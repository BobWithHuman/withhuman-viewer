//! The developer's hooks: the engine's agent setup (`client/`, from withhuman-core's
//! `deploy/client`), written to the app's settings folder and run with Python. Claude Code is set
//! up for one folder or every folder of the user; the other agents the setup knows each as it
//! takes it: Codex, Cursor, Copilot and Antigravity in a project folder, Hermes and OpenClaw in
//! the user's own profile of theirs. The setup itself writes the hooks, the device's client ID
//! and its registration with the engine; the app only runs it and reads what it printed.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The setup's files, by their place beside `configure.py` (scripts/sync-client.sh copies them).
const FILES: &[(&str, &str)] = &[
    ("configure.py", include_str!("../client/configure.py")),
    (
        "hooks/pre-tool-use.py",
        include_str!("../client/hooks/pre-tool-use.py"),
    ),
    (
        "hooks/withhuman_bridge.py",
        include_str!("../client/hooks/withhuman_bridge.py"),
    ),
    (
        "antigravity/withhuman-hook.py",
        include_str!("../client/antigravity/withhuman-hook.py"),
    ),
    (
        "antigravity/antigravity_transcript.py",
        include_str!("../client/antigravity/antigravity_transcript.py"),
    ),
    (
        "codex/withhuman-hook.py",
        include_str!("../client/codex/withhuman-hook.py"),
    ),
    (
        "codex/codex_transcript.py",
        include_str!("../client/codex/codex_transcript.py"),
    ),
    (
        "copilot/withhuman-hook.py",
        include_str!("../client/copilot/withhuman-hook.py"),
    ),
    (
        "copilot/copilot_transcript.py",
        include_str!("../client/copilot/copilot_transcript.py"),
    ),
    (
        "cursor/withhuman-hook.py",
        include_str!("../client/cursor/withhuman-hook.py"),
    ),
    (
        "hermes/__init__.py",
        include_str!("../client/hermes/__init__.py"),
    ),
    (
        "hermes/plugin.yaml",
        include_str!("../client/hermes/plugin.yaml"),
    ),
    (
        "hermes/withhuman-hook.py",
        include_str!("../client/hermes/withhuman-hook.py"),
    ),
    (
        "openclaw/index.js",
        include_str!("../client/openclaw/index.js"),
    ),
    (
        "openclaw/openclaw.plugin.json",
        include_str!("../client/openclaw/openclaw.plugin.json"),
    ),
    (
        "openclaw/package.json",
        include_str!("../client/openclaw/package.json"),
    ),
    (
        "openclaw/withhuman-hook.py",
        include_str!("../client/openclaw/withhuman-hook.py"),
    ),
];

/// A coding agent the setup knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
    Copilot,
    Antigravity,
    Hermes,
    OpenClaw,
}

impl Agent {
    pub const ALL: [Agent; 7] = [
        Agent::Claude,
        Agent::Codex,
        Agent::Cursor,
        Agent::Copilot,
        Agent::Antigravity,
        Agent::Hermes,
        Agent::OpenClaw,
    ];

    /// The setup's name for it (`--agent`).
    pub fn kind(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Cursor => "cursor",
            Agent::Copilot => "copilot",
            Agent::Antigravity => "antigravity",
            Agent::Hermes => "hermes",
            Agent::OpenClaw => "openclaw",
        }
    }

    pub fn from_kind(kind: &str) -> Option<Agent> {
        Agent::ALL.into_iter().find(|a| a.kind() == kind)
    }

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
            Agent::Cursor => "Cursor",
            Agent::Copilot => "Copilot",
            Agent::Antigravity => "Antigravity",
            Agent::Hermes => "Hermes",
            Agent::OpenClaw => "OpenClaw",
        }
    }

    /// Set up one project folder at a time (Claude Code can be either).
    pub fn per_folder(self) -> bool {
        matches!(
            self,
            Agent::Codex | Agent::Cursor | Agent::Copilot | Agent::Antigravity
        )
    }

    /// Set up once for the user, in its own profile, whatever the folder.
    pub fn per_profile(self) -> bool {
        matches!(self, Agent::Hermes | Agent::OpenClaw)
    }

    /// The commands it is run by, and the folder in the home it keeps its own state in.
    fn marks(self) -> (&'static [&'static str], Option<&'static str>) {
        match self {
            Agent::Claude => (&["claude"], Some(".claude")),
            Agent::Codex => (&["codex"], Some(".codex")),
            Agent::Cursor => (&["cursor-agent", "cursor"], Some(".cursor")),
            Agent::Copilot => (&["copilot"], Some(".copilot")),
            Agent::Antigravity => (&["antigravity"], Some(".antigravity")),
            Agent::Hermes => (&["hermes"], Some(".hermes")),
            Agent::OpenClaw => (&["openclaw"], Some(".openclaw")),
        }
    }

    /// Whether it is on this machine: its command on `PATH` or where installers put one, or
    /// its folder in the home.
    pub fn found(self) -> bool {
        let (commands, folder) = self.marks();
        let home = dirs::home_dir();
        if let (Some(home), Some(folder)) = (&home, folder)
            && home.join(folder).is_dir()
        {
            return true;
        }
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
        dirs.extend(python_dirs());
        if let Some(home) = &home {
            dirs.extend(
                [".local/bin", ".npm-global/bin", ".bun/bin", ".cargo/bin"].map(|d| home.join(d)),
            );
        }
        dirs.iter().any(|d| {
            commands.iter().any(|c| {
                let exe = if cfg!(windows) {
                    format!("{c}.exe")
                } else {
                    (*c).to_owned()
                };
                d.join(exe).is_file() || (cfg!(windows) && d.join(format!("{c}.cmd")).is_file())
            })
        })
    }

    /// The file a project folder's setup of it is in.
    fn folder_file(self, folder: &Path) -> PathBuf {
        folder.join(match self {
            Agent::Claude => ".claude/settings.json",
            Agent::Codex => ".codex/hooks.json",
            Agent::Cursor => ".cursor/hooks.json",
            Agent::Copilot => ".github/hooks/withhuman-copilot.json",
            Agent::Antigravity => ".agents/hooks.json",
            Agent::Hermes | Agent::OpenClaw => "",
        })
    }

    /// Where the user's setup of an agent set up in its own profile is.
    fn profile_dir(self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        match self {
            Agent::Hermes => Some(
                std::env::var_os("HERMES_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".hermes"))
                    .join("plugins/withhuman"),
            ),
            Agent::OpenClaw => Some(
                std::env::var_os("OPENCLAW_STATE_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".openclaw"))
                    .join("withhuman-setup"),
            ),
            _ => None,
        }
    }
}

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
    let stub = mac_stub();
    for dir in std::env::split_paths(&path).chain(python_dirs()) {
        if stub && dir == Path::new("/usr/bin") {
            continue;
        }
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

/// Whether macOS's `/usr/bin/python3` is only the stand-in that asks to install Apple's command
/// line tools: run, it opens that dialog, so it is not tried while they are not installed.
fn mac_stub() -> bool {
    cfg!(target_os = "macos")
        && !Command::new("/usr/bin/xcode-select")
            .arg("-p")
            .output()
            .is_ok_and(|o| o.status.success())
}

/// The Python the hooks would run with, if there is one.
pub fn python_found() -> Option<PathBuf> {
    python().ok()
}

/// Asks for a Python: on a Mac, Apple's command line tools, which carry one; elsewhere
/// python.org's download page.
pub fn get_python() {
    if cfg!(target_os = "macos") {
        let _ = Command::new("/usr/bin/xcode-select")
            .arg("--install")
            .spawn();
    } else {
        let opener = if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        let _ = command(opener)
            .arg("https://www.python.org/downloads/")
            .spawn();
    }
}

/// The setup's files, written where the app keeps its own, each where the setup looks for it.
fn files() -> Result<PathBuf, String> {
    let dir = crate::api::config_dir().join("client");
    for (name, text) in FILES {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("설치 파일을 쓰지 못했습니다: {e}"))?;
        }
        std::fs::write(&path, text).map_err(|e| format!("설치 파일을 쓰지 못했습니다: {e}"))?;
    }
    Ok(dir)
}

/// The setup's arguments: where, against which engine, with the user's key when given; or
/// taking a setup out. The developer's own engine on this machine (`own_engine`) lets calls go
/// on while it is away, for a setup of every folder of the user: the setup refuses it elsewhere.
fn arguments(
    scope: Scope,
    folder: Option<&Path>,
    server: &str,
    key: Option<&str>,
    uninstall: bool,
    own_engine: bool,
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
    if own_engine && scope == Scope::Everywhere {
        args.push("--on-unreachable".into());
        args.push("allow".into());
    }
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
    own_engine: bool,
) -> Result<Done, String> {
    let args = arguments(scope, folder, server, key, uninstall, own_engine)?;
    let out = setup(&args, scope == Scope::Device)?;
    Ok(Done {
        client_id: client_id(&out),
    })
}

/// Sets up an agent other than Claude Code, or takes its setup out (`uninstall`): in `folder`
/// for one set up per project folder, in the user's own profile of it otherwise. Blocks while the
/// setup runs. Its hooks deny while the engine is away: only Claude Code's setup of every folder
/// may let calls go on.
pub fn run_agent(
    agent: Agent,
    folder: Option<&Path>,
    server: &str,
    key: Option<&str>,
    uninstall: bool,
) -> Result<Done, String> {
    let args = agent_arguments(agent, folder, server, key, uninstall)?;
    let out = setup(&args, false)?;
    Ok(Done {
        client_id: client_id(&out),
    })
}

fn agent_arguments(
    agent: Agent,
    folder: Option<&Path>,
    server: &str,
    key: Option<&str>,
    uninstall: bool,
) -> Result<Vec<String>, String> {
    let mut args = vec!["--agent".to_owned(), agent.kind().to_owned()];
    if agent.per_folder() {
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
    /// The user's own profile of an agent (Hermes's, OpenClaw's), for every folder.
    Profile,
}

/// A WithHuman setup found on this machine: of which agent, where, its settings file, and the
/// engine its hooks report to.
#[derive(Clone, Debug, PartialEq)]
pub struct Install {
    pub agent: Agent,
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
    Some(Install {
        agent: Agent::Claude,
        at,
        at_ms: modified(&file),
        file,
        engine,
    })
}

fn modified(file: &Path) -> Option<i64> {
    std::fs::metadata(file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

/// The first engine address in `text`: what an agent's hooks file, or the `server` file beside
/// its hook, names.
fn address_in(text: &str) -> Option<String> {
    let start = text.find("http://").or_else(|| text.find("https://"))?;
    let rest = &text[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ','))
        .unwrap_or(rest.len());
    let url = &rest[..end];
    Some(
        url.split("/v1/hooks")
            .next()
            .unwrap_or(url)
            .trim_end_matches('/')
            .to_owned(),
    )
}

/// Another agent's setup in `folder`, if its hooks file holds WithHuman's hooks.
fn found_agent(agent: Agent, folder: &Path) -> Option<Install> {
    let file = agent.folder_file(folder);
    let text = std::fs::read_to_string(&file).ok()?;
    if !text.contains("withhuman") {
        return None;
    }
    // Cursor keeps the address beside its hook, not in the file.
    let engine = address_in(&text).or_else(|| {
        let beside = file.parent()?.join("withhuman/server");
        address_in(&std::fs::read_to_string(beside).ok()?)
    });
    Some(Install {
        agent,
        at: Where::Folder(folder.to_path_buf()),
        at_ms: modified(&file),
        file,
        engine,
    })
}

/// The user's setup of an agent set up in its own profile.
fn found_profile(agent: Agent) -> Option<Install> {
    let dir = agent.profile_dir()?;
    let hook = dir.join("withhuman-hook.py");
    if !hook.is_file() {
        return None;
    }
    let engine = std::fs::read_to_string(dir.join("server"))
        .ok()
        .and_then(|t| address_in(&t));
    Some(Install {
        agent,
        at: Where::Profile,
        at_ms: modified(&hook),
        file: dir,
        engine,
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
    for agent in Agent::ALL.into_iter().filter(|a| a.per_profile()) {
        out.extend(found_profile(agent));
    }
    for folder in folders {
        let folder = PathBuf::from(folder);
        let file = folder.join(".claude/settings.json");
        out.extend(found(Where::Folder(folder.clone()), file));
        for agent in Agent::ALL.into_iter().filter(|a| a.per_folder()) {
            out.extend(found_agent(agent, &folder));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn another_agent_is_set_up_as_the_setup_takes_it() {
        let folder = Path::new("/work/app");
        assert_eq!(
            agent_arguments(Agent::Codex, Some(folder), " http://e:3000 ", None, false).unwrap(),
            [
                "--agent",
                "codex",
                "--project",
                "/work/app",
                "--server",
                "http://e:3000"
            ]
        );
        assert!(agent_arguments(Agent::Cursor, None, "http://e", None, false).is_err());
        assert_eq!(
            agent_arguments(Agent::Hermes, None, "http://e", Some("whk_x"), false).unwrap(),
            [
                "--agent", "hermes", "--server", "http://e", "--key", "whk_x"
            ]
        );
        assert_eq!(
            agent_arguments(Agent::OpenClaw, None, "", None, true).unwrap(),
            ["--agent", "openclaw", "--uninstall"]
        );
        // Every file the setup copies for an agent is one the app carries.
        for agent in [
            "antigravity",
            "codex",
            "copilot",
            "cursor",
            "hermes",
            "openclaw",
        ] {
            assert!(
                FILES
                    .iter()
                    .any(|(name, _)| name.starts_with(&format!("{agent}/"))),
                "{agent}"
            );
        }
        assert_eq!(
            address_in(
                r#"{"command": "\"/py\" \"/h/withhuman-hook.py\" http://127.0.0.1:47380 PreToolUse"}"#
            ),
            Some("http://127.0.0.1:47380".into())
        );
        assert_eq!(
            address_in("http://e:3000/v1/hooks/x\n"),
            Some("http://e:3000".into())
        );
    }

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
                false,
                true
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
            arguments(
                Scope::Everywhere,
                None,
                "http://e",
                Some("whk_x"),
                false,
                false
            )
            .unwrap(),
            ["--scope", "user", "--server", "http://e", "--key", "whk_x"]
        );
        assert_eq!(
            arguments(Scope::Everywhere, None, "", None, true, true).unwrap(),
            ["--scope", "user", "--uninstall"]
        );
        // The device's setup runs no other hooks beside it, and takes no one's key.
        assert_eq!(
            arguments(Scope::Device, None, "http://e", Some("whk_x"), false, true).unwrap(),
            [
                "--scope",
                "managed",
                "--managed-hooks-only",
                "--server",
                "http://e"
            ]
        );
        assert_eq!(
            arguments(Scope::Device, None, "", None, true, false).unwrap(),
            ["--scope", "managed", "--uninstall"]
        );
        assert!(arguments(Scope::Folder, None, "http://e", None, false, false).is_err());
        assert!(arguments(Scope::Everywhere, None, " ", None, false, false).is_err());
        // The developer's own engine lets calls go on while it is away, for every folder only.
        assert_eq!(
            arguments(Scope::Everywhere, None, "http://e", None, false, true).unwrap(),
            [
                "--scope",
                "user",
                "--server",
                "http://e",
                "--on-unreachable",
                "allow"
            ]
        );
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
