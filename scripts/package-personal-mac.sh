#!/usr/bin/env bash
# Packages the personal app for macOS with its own engine inside (src/own.rs): the viewer and
# withhuman-core's `withhuman-local`, both universal, side by side in Contents/MacOS where the app
# looks for them. PostgreSQL is inside the engine (its `bundled` feature), so the first launch
# installs it without the network. The app has its own name and identifier (dev.withhuman.personal), so it installs beside
# the security team's. cargo-packager's settings in Cargo.toml stay the security team's app's.
#
# The engine is built at the version `engine.ref` names (a withhuman-core tag or commit), from a
# clean checkout of it in target/, so a release carries the engine it was tested with.
# WITHHUMAN_CORE instead builds a checkout as it is, for trying an engine change.
# SIGNING_IDENTITY, when set, signs the app as the release does; with the APPLE_* variables of
# the release set too, cargo-packager has Apple notarize it, and with CARGO_PACKAGER_SIGN_PRIVATE_KEY
# it archives and signs it for the updater.
# usage: scripts/package-personal-mac.sh   → dist/WithHuman Personal.app and its disk image
set -euo pipefail
cd "$(dirname "$0")/.."

core=$(scripts/engine-checkout.sh)

out=target/universal-apple-darwin/release
mkdir -p "$out"
for target in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target add "$target" >/dev/null
  POSTGRESQL_VERSION="=16.16.0" cargo build --release --locked --manifest-path "$core/Cargo.toml" \
    --target "$target" -p withhuman-local --features bundled
done
lipo -create -output "$out/withhuman-local" \
  "$core/target/aarch64-apple-darwin/release/withhuman-local" \
  "$core/target/x86_64-apple-darwin/release/withhuman-local"
lipo -info "$out/withhuman-local"

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
signing=""
if [ -n "${SIGNING_IDENTITY:-}" ]; then
  signing="\"signingIdentity\": \"$SIGNING_IDENTITY\","
fi
# At the root: cargo-packager reads the paths in it from the file's own folder.
config=packager-personal-mac.json
trap 'rm -f "$config"' EXIT
cat > "$config" <<JSON
{
  "name": "withhuman-viewer",
  "productName": "WithHuman Personal",
  "version": "$version",
  "identifier": "dev.withhuman.personal",
  "description": "WithHuman 개인 앱",
  "homepage": "https://github.com/BobWithHuman/withhuman-viewer",
  "publisher": "BobWithHuman",
  "copyright": "Copyright (c) 2026 BobWithHuman",
  "category": "DeveloperTool",
  "outDir": "dist",
  "formats": ["app", "dmg"],
  "binariesDir": "$out",
  "binaries": [
    { "path": "withhuman-viewer", "main": true },
    { "path": "withhuman-local" }
  ],
  "beforePackagingCommand": "VIEWER_APP=personal scripts/build-universal.sh",
  "icons": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns"],
  "macos": { $signing "minimumSystemVersion": "11.0" }
}
JSON
cargo packager --release --target universal-apple-darwin --config "$config"
ls "dist/WithHuman Personal.app/Contents/MacOS"
