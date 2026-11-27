#!/usr/bin/env bash
# Builds the viewer for Apple Silicon and Intel, and joins the two into one universal binary
# where cargo-packager looks for it (target/universal-apple-darwin/release).
# VIEWER_APP picks the app: `personal` (the default, the developer's own) or `admin` (the security
# team's).
set -euo pipefail
cd "$(dirname "$0")/.."

case "${VIEWER_APP:-personal}" in
  personal) features=() ;;
  admin) features=(--no-default-features --features admin) ;;
  *) echo "VIEWER_APP is personal or admin, not ${VIEWER_APP}" >&2; exit 1 ;;
esac

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target add "$target" >/dev/null
  cargo build --release --locked --target "$target" ${features[@]+"${features[@]}"}
done

out=target/universal-apple-darwin/release
mkdir -p "$out"
lipo -create -output "$out/withhuman-viewer" \
  target/aarch64-apple-darwin/release/withhuman-viewer \
  target/x86_64-apple-darwin/release/withhuman-viewer
lipo -info "$out/withhuman-viewer"
