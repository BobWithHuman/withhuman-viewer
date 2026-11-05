#!/usr/bin/env bash
# Builds the viewer for Apple Silicon and Intel, and joins the two into one universal binary
# where cargo-packager looks for it (target/universal-apple-darwin/release).
set -euo pipefail
cd "$(dirname "$0")/.."

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target add "$target" >/dev/null
  cargo build --release --locked --target "$target"
done

out=target/universal-apple-darwin/release
mkdir -p "$out"
lipo -create -output "$out/withhuman-viewer" \
  target/aarch64-apple-darwin/release/withhuman-viewer \
  target/x86_64-apple-darwin/release/withhuman-viewer
lipo -info "$out/withhuman-viewer"
