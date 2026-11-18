#!/usr/bin/env bash
# Copies the engine's Claude Code setup (configure.py and its PreToolUse hook) from a
# withhuman-core checkout into client/, which the developer's app installs hooks with.
# usage: scripts/sync-client.sh [path to withhuman-core]   (default: ../withhuman-core)
set -euo pipefail
cd "$(dirname "$0")/.."
core=${1:-../withhuman-core}
cp "$core/deploy/client/configure.py" client/configure.py
cp "$core/deploy/client/hooks/pre-tool-use.py" client/hooks/pre-tool-use.py
echo "client/ from $(git -C "$core" rev-parse --short HEAD) ($(git -C "$core" rev-parse --abbrev-ref HEAD))"
