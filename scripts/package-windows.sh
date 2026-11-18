#!/usr/bin/env bash
# Packages the developer's app for Windows: an NSIS installer in dist/, with the Start menu entry
# and uninstaller Windows expects. Run from Git Bash on Windows (as CI and the release do).
# cargo-packager's settings in Cargo.toml are the macOS app's, so these are its own.
# usage: scripts/package-windows.sh
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
config=target/packager-windows.json
mkdir -p target
cat > "$config" <<JSON
{
  "name": "withhuman-viewer",
  "productName": "WithHuman Dev",
  "version": "$version",
  "identifier": "dev.withhuman.viewer",
  "description": "WithHuman 개발자 앱",
  "publisher": "BobWithHuman",
  "copyright": "Copyright (c) 2026 BobWithHuman",
  "category": "DeveloperTool",
  "outDir": "dist",
  "formats": ["nsis"],
  "binariesDir": "target/release",
  "binaries": [{ "path": "withhuman-viewer", "main": true }],
  "beforePackagingCommand": "cargo build --release --locked",
  "icons": ["icons/icon.ico", "icons/32x32.png", "icons/128x128.png"]
}
JSON
cargo packager --release --config "$config"
ls dist/*.exe
