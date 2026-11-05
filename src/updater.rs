//! Updates from GitHub Releases. Each release carries `latest.json`, naming the version and the
//! signed app archive; the archive is installed only when its signature matches the public key
//! below, whose private half lives in the repository's release secrets.

use std::path::PathBuf;

use cargo_packager_updater::semver::Version;
use cargo_packager_updater::url::Url;
use cargo_packager_updater::{Config, Update};

/// The minisign public key the update archives are signed with.
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDQyOTJFMkU0OUE0RUM2RUUKUldUdXhrNmE1T0tTUWpzcmVNbDMvQmRGTDc0S0hOUXNqSGMrZUxVS1VMbmtyTWoyUkdJT0xGMG4K";

/// The newest published release's manifest; drafts and pre-releases are not "latest".
const ENDPOINT: &str =
    "https://github.com/BobWithHuman/withhuman-viewer/releases/latest/download/latest.json";

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `.app` this process runs from, or `None` when it runs outside one (`cargo run`), where
/// there is nothing to replace.
pub fn app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bundle = exe.parent()?.parent()?.parent()?;
    let is_bundle =
        exe.parent()?.ends_with("Contents/MacOS") && bundle.extension().is_some_and(|e| e == "app");
    is_bundle.then(|| bundle.to_path_buf())
}

/// Whether this copy can update itself: an installed app, not turned off by `VIEWER_NO_UPDATE=1`.
pub fn enabled() -> bool {
    app_bundle().is_some() && std::env::var("VIEWER_NO_UPDATE").map_or(true, |v| v != "1")
}

/// Asks GitHub for a newer version.
pub async fn check() -> Result<Option<Update>, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config {
            endpoints: vec![Url::parse(ENDPOINT).expect("valid endpoint")],
            pubkey: PUBKEY.to_owned(),
            ..Config::default()
        };
        let current = Version::parse(VERSION).expect("the package version is semver");
        cargo_packager_updater::check_update(current, config)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("업데이트를 확인하지 못했습니다: {e}"))
}

/// Downloads the update, checks its signature, and puts it in place of the running app.
pub async fn install(update: Update) -> Result<(), String> {
    tokio::task::spawn_blocking(move || update.download_and_install())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("업데이트를 설치하지 못했습니다: {e}"))
}

/// Opens the freshly installed app and ends this one.
pub fn relaunch() {
    if let Some(bundle) = app_bundle() {
        let _ = std::process::Command::new("open")
            .arg("-n")
            .arg(&bundle)
            .spawn();
    }
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_and_endpoint_read() {
        assert!(Url::parse(ENDPOINT).is_ok());
        assert!(Version::parse(VERSION).is_ok());
        // The key is base64 of a minisign public key file.
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, PUBKEY)
            .expect("base64");
        assert!(String::from_utf8_lossy(&decoded).contains("minisign public key"));
    }

    #[test]
    fn tests_do_not_run_inside_an_app() {
        assert!(app_bundle().is_none());
    }
}
