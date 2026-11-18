#!/usr/bin/env bash
# Packages one of the two apps for Windows as an NSIS installer, with the Start menu entry and
# uninstaller Windows expects: `dev`, the developer's (WithHuman Dev), or `admin`, the security
# team's (WithHuman Viewer, as on macOS). Each has its own identifier, so both can be installed.
# Run from Git Bash on Windows (as CI and the release do). cargo-packager's settings in
# Cargo.toml are the macOS app's, so these are its own.
# usage: scripts/package-windows.sh dev|admin   → dist/WithHuman-{Dev,Viewer}_<version>_x64-setup.exe
set -euo pipefail
cd "$(dirname "$0")/.."
app=${1:?usage: scripts/package-windows.sh dev|admin}
case "$app" in
  dev)
    product="WithHuman Dev"; file="WithHuman-Dev"; identifier="dev.withhuman.dev"
    description="WithHuman 개발자 앱"; features="" ;;
  admin)
    product="WithHuman Viewer"; file="WithHuman-Viewer"; identifier="dev.withhuman.viewer"
    description="WithHuman 보안팀 앱"; features="--no-default-features --features admin" ;;
  *) echo "usage: scripts/package-windows.sh dev|admin" >&2; exit 2 ;;
esac
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
# At the root: cargo-packager reads the paths in it from the file's own folder.
config=packager-windows.json
trap 'rm -f "$config"' EXIT
cat > "$config" <<JSON
{
  "name": "withhuman-viewer",
  "productName": "$product",
  "version": "$version",
  "identifier": "$identifier",
  "description": "$description",
  "publisher": "BobWithHuman",
  "copyright": "Copyright (c) 2026 BobWithHuman",
  "category": "DeveloperTool",
  "outDir": "dist",
  "formats": ["nsis"],
  "binariesDir": "target/release",
  "binaries": [{ "path": "withhuman-viewer", "main": true }],
  "beforePackagingCommand": "cargo build --release --locked $features",
  "icons": ["icons/icon.ico", "icons/32x32.png", "icons/128x128.png"]
}
JSON
cargo packager --release --config "$config"
# A name without spaces, as the macOS files have.
mv "dist/${product}_${version}_x64-setup.exe" "dist/${file}_${version}_x64-setup.exe"
ls -l "dist/${file}_${version}_x64-setup.exe"
