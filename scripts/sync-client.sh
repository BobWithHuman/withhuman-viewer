#!/usr/bin/env bash
# Copies the engine's agent setup from a withhuman-core checkout into client/, which the personal
# app installs hooks with: configure.py, Claude Code's hook and the bridge they share, and each
# other agent's hook folder (src/hooks.rs lists them). Tests and fixtures stay behind.
# usage: scripts/sync-client.sh [path to withhuman-core]   (default: ../withhuman-core)
set -euo pipefail
cd "$(dirname "$0")/.."
core=${1:-../withhuman-core}
src="$core/deploy/client"
cp "$src/configure.py" client/configure.py
mkdir -p client/hooks
cp "$src/hooks/pre-tool-use.py" "$src/hooks/withhuman_bridge.py" client/hooks/
for agent in antigravity codex copilot cursor hermes openclaw; do
  rm -rf "client/$agent"
  mkdir -p "client/$agent"
  find "$src/$agent" -maxdepth 1 -type f ! -name 'test_*' -exec cp {} "client/$agent/" \;
done
echo "client/ from $(git -C "$core" rev-parse --short HEAD) ($(git -C "$core" rev-parse --abbrev-ref HEAD))"
