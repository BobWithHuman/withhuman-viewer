#!/usr/bin/env bash
# Prints the withhuman-core checkout the personal app's engine is built from: WITHHUMAN_CORE as it
# is, else a clean checkout in target/withhuman-core of the version engine.ref names (a tag or a
# commit), so a release carries the engine it was tested with. Sourced by the package scripts.
set -euo pipefail
cd "$(dirname "$0")/.."
if [ -n "${WITHHUMAN_CORE:-}" ]; then
  core=$WITHHUMAN_CORE
  echo "engine: $core as it is ($(git -C "$core" describe --always --dirty 2>/dev/null || echo '?'))" >&2
else
  if [ ! -s engine.ref ]; then
    echo "engine.ref names no withhuman-core version; write a tag or commit in it, or set WITHHUMAN_CORE" >&2
    exit 1
  fi
  ref=$(tr -d '[:space:]' < engine.ref)
  core=target/withhuman-core
  if [ ! -d "$core/.git" ]; then
    git clone --quiet https://github.com/BobWithHuman/withhuman-core.git "$core"
  fi
  git -C "$core" fetch --quiet --tags origin
  git -C "$core" checkout --quiet --force --detach "$ref"
  git -C "$core" clean --quiet -fdx -e target
  echo "engine: withhuman-core $ref ($(git -C "$core" rev-parse --short HEAD))" >&2
fi
if [ ! -f "$core/services/local/Cargo.toml" ]; then
  echo "$core has no services/local: withhuman-local is in withhuman-core from its personal profile on" >&2
  exit 1
fi
echo "$core"
