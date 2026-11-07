#!/bin/sh
# Cargo's runner on macOS (see .cargo/config.toml). Before `cargo run` starts the app, it signs
# the build with one fixed identity, so the keychain's "always allow" for the saved Neo4j
# password holds from one build to the next instead of asking after every build.
#
# The identity is VIEWER_SIGN_IDENTITY, or the first "Developer ID Application" one in the
# keychain. Without one the app starts unsigned, as before. Test binaries are left alone.
bin="$1"
if [ "$(basename "$bin")" = withhuman-viewer ]; then
  id="${VIEWER_SIGN_IDENTITY:-$(security find-identity -v -p codesigning 2>/dev/null |
    sed -n 's/.*"\(Developer ID Application:[^"]*\)".*/\1/p' | head -n 1)}"
  if [ -n "$id" ]; then
    codesign --force --timestamp=none --identifier dev.withhuman.viewer --sign "$id" "$bin" 2>/dev/null ||
      echo "dev-run: could not sign $bin with \"$id\"; it starts unsigned" >&2
  fi
fi
exec "$@"
