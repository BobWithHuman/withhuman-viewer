//! What the first launch sets up: the Jev key, checked against Jev before it is kept, and the
//! engine, either started on this Mac from its folder (withhuman-core) or reached on a server.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::json;

/// Where the engine asks Jev, as withhuman-core's decision service does (`JEV_URL`).
fn jev_url() -> String {
    std::env::var("JEV_URL").unwrap_or_else(|_| "https://api.typesafe.ai".into())
}

/// Whether Jev takes the key. A request with no questions costs nothing: Jev answers 401 for a
/// key it does not know and 422 (no questions) for one it does, the key being checked first.
pub async fn check_jev(client: &reqwest::Client, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("Jev API 키를 넣어 주세요.".into());
    }
    let url = format!("{}/v1/systemone", jev_url().trim_end_matches('/'));
    let response = client
        .post(&url)
        .bearer_auth(key)
        .json(&json!({"model": "jev-1.14.0", "state": {}, "questions": {}}))
        .send()
        .await
        .map_err(|e| format!("Jev({url})에 닿지 못했습니다. 인터넷 연결을 확인해 주세요. ({e})"))?;
    match response.status().as_u16() {
        200..=299 | 400 | 422 => Ok(()),
        401 | 403 => Err("Jev가 이 키를 받지 않았습니다. 키를 다시 확인해 주세요.".into()),
        429 => Err("Jev에 요청이 너무 많습니다. 잠시 뒤 다시 확인해 주세요.".into()),
        status => Err(format!(
            "Jev가 예상하지 못한 응답을 보냈습니다 (HTTP {status})."
        )),
    }
}

/// The engine's folder on this Mac: `WITHHUMAN_CORE`, else `~/withhuman-core`, if it holds the
/// engine's compose file.
pub fn find_dir() -> Option<PathBuf> {
    let home = dirs::home_dir();
    std::env::var_os("WITHHUMAN_CORE")
        .map(PathBuf::from)
        .into_iter()
        .chain(home.map(|h| h.join("withhuman-core")))
        .find(|d| is_engine_dir(d))
}

/// A folder with withhuman-core's compose file for a personal Docker host.
pub fn is_engine_dir(dir: &Path) -> bool {
    fs::read_to_string(dir.join("compose.yaml"))
        .is_ok_and(|t| t.contains("decision:") && t.contains("generator:"))
}

/// `KEY=VALUE` lines of the folder's `.env`, without comments or quotes.
pub fn read_env(dir: &Path) -> HashMap<String, String> {
    let text = fs::read_to_string(dir.join(".env")).unwrap_or_default();
    parse_env(&text)
}

fn parse_env(text: &str) -> HashMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("export ").unwrap_or(l).split_once('='))
        .map(|(k, v)| {
            let v = v.trim();
            let v = v
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(v);
            (k.trim().to_owned(), v.to_owned())
        })
        .collect()
}

/// The port Neo4j's HTTP is published on, on loopback, by the folder's own override file.
fn published_port(dir: &Path) -> Option<u16> {
    let text = fs::read_to_string(dir.join("compose.override.yaml")).ok()?;
    text.lines().find_map(|l| {
        let l = l.trim().trim_start_matches('-').trim().trim_matches('"');
        let rest = l.strip_prefix("127.0.0.1:")?;
        let (port, target) = rest.split_once(':')?;
        (target == "7474").then(|| port.parse().ok()).flatten()
    })
}

/// The port the viewer reads Neo4j on when it starts the engine itself: the folder's override
/// file's, else the viewer's own override (7476).
pub fn neo4j_port(dir: &Path) -> u16 {
    published_port(dir).unwrap_or(VIEWER_PORT)
}

const VIEWER_PORT: u16 = 7476;

/// Docker's command line: a Mac app started from Finder does not get the shell's PATH.
fn docker() -> Option<PathBuf> {
    let found = std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join("docker"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    found
        .into_iter()
        .chain(
            [
                "/usr/local/bin/docker",
                "/opt/homebrew/bin/docker",
                "/Applications/Docker.app/Contents/Resources/bin/docker",
            ]
            .map(PathBuf::from),
        )
        .find(|p| p.is_file())
}

/// What starting the engine needs besides the Jev key, read from the folder: its `.env`'s
/// image tag and Neo4j password, with withhuman-core's defaults when it has none.
pub struct Local {
    pub dir: PathBuf,
    pub password: String,
    pub port: u16,
    image_tag: String,
}

impl Local {
    pub fn read(dir: &Path) -> Result<Self, String> {
        if !is_engine_dir(dir) {
            return Err(format!(
                "{}에서 엔진(withhuman-core)의 compose.yaml을 찾지 못했습니다.",
                dir.display()
            ));
        }
        let env = read_env(dir);
        let get = |k: &str, d: &str| {
            env.get(k)
                .filter(|v| !v.is_empty())
                .cloned()
                .unwrap_or_else(|| d.to_owned())
        };
        Ok(Self {
            dir: dir.to_owned(),
            password: get("NEO4J_PASSWORD", "withhuman-e2e"),
            port: neo4j_port(dir),
            image_tag: get("IMAGE_TAG", "edge"),
        })
    }

    /// `docker compose up -d` in the folder, the Jev key given to the decision service. Each
    /// line Docker prints goes to `log`. Blocks until Docker is done.
    pub fn up(&self, jev_key: &str, log: &Arc<Mutex<Vec<String>>>) -> Result<(), String> {
        let docker = docker().ok_or(
            "Docker를 찾지 못했습니다. Docker Desktop을 설치하고 켠 다음 다시 시도해 주세요.",
        )?;
        let mut files = vec!["compose.yaml".to_owned()];
        if self.dir.join("compose.override.yaml").is_file() {
            files.push("compose.override.yaml".into());
        }
        if published_port(&self.dir).is_none() {
            files.push(viewer_override()?.display().to_string());
        }
        let mut args = vec!["compose".to_owned()];
        for f in &files {
            args.push("-f".into());
            args.push(f.clone());
        }
        args.extend(["up", "-d"].map(String::from));
        push(log, format!("$ docker {}", args.join(" ")));
        let mut child = Command::new(&docker)
            .args(&args)
            .current_dir(&self.dir)
            .env("JEV_API_KEY", jev_key.trim())
            .env("NEO4J_PASSWORD", &self.password)
            .env("IMAGE_TAG", &self.image_tag)
            // Docker's helpers (credentials, compose) sit beside it.
            .env("PATH", path_with(&docker))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Docker를 실행하지 못했습니다: {e}"))?;
        // Compose reports progress on stderr; read both so neither pipe fills up.
        let out = child.stdout.take().map(|s| {
            let log = log.clone();
            std::thread::spawn(move || {
                BufReader::new(s)
                    .lines()
                    .map_while(Result::ok)
                    .for_each(|l| push(&log, l))
            })
        });
        if let Some(err) = child.stderr.take() {
            BufReader::new(err)
                .lines()
                .map_while(Result::ok)
                .for_each(|l| push(log, l));
        }
        if let Some(t) = out {
            let _ = t.join();
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            let last = log
                .lock()
                .map(|l| l.last().cloned().unwrap_or_default())
                .unwrap_or_default();
            Err(format!(
                "엔진을 띄우지 못했습니다. Docker Desktop이 켜져 있는지 확인해 주세요. ({last})"
            ))
        }
    }
}

fn push(log: &Arc<Mutex<Vec<String>>>, line: String) {
    let line = line.trim().to_owned();
    if !line.is_empty()
        && let Ok(mut l) = log.lock()
    {
        l.push(line);
    }
}

/// PATH with Docker's own folder and the usual tool folders in front.
fn path_with(docker: &Path) -> String {
    let mut dirs: Vec<PathBuf> = docker.parent().map(Path::to_path_buf).into_iter().collect();
    dirs.extend(["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    if let Some(p) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    std::env::join_paths(dirs)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The viewer's own override for a folder without one: Neo4j's HTTP on loopback, for the viewer.
fn viewer_override() -> Result<PathBuf, String> {
    let path = crate::api::config_dir().join("engine.override.yaml");
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = format!(
        "# Written by WithHuman Viewer: Neo4j's HTTP port on loopback, for the viewer.\nservices:\n  neo4j:\n    ports:\n      - \"127.0.0.1:{VIEWER_PORT}:7474\"\n"
    );
    fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_env_file_is_read_without_comments_or_quotes() {
        let env = parse_env(
            "# c\nIMAGE_TAG=edge\nNEO4J_PASSWORD=\"p w\"\nexport JEV_API_KEY='k'\n\nBAD\n",
        );
        assert_eq!(env["IMAGE_TAG"], "edge");
        assert_eq!(env["NEO4J_PASSWORD"], "p w");
        assert_eq!(env["JEV_API_KEY"], "k");
        assert!(!env.contains_key("BAD"));
    }

    #[test]
    fn the_published_port_comes_from_the_override() {
        let dir = std::env::temp_dir().join(format!("wh-engine-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("compose.override.yaml"),
            "services:\n  neo4j:\n    ports:\n      - \"127.0.0.1:7476:7474\"\n",
        )
        .unwrap();
        assert_eq!(published_port(&dir), Some(7476));
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(published_port(&dir), None);
    }
}
