#!/usr/bin/env bash
# End-to-end test of the npm launcher in packaging/npm.
#
#   1. a fresh cache: `node bin/re.js --version` downloads the release that
#      matches package.json, verifies it against SHA256SUMS.txt and prints
#      the version (network required);
#   2. the second run is a cache hit: no download;
#   3. a mirror whose archive does not match its SHA256SUMS.txt is refused,
#      and nothing lands in the cache;
#   4. a mirror whose SHA256SUMS.txt lacks the asset is refused;
#   5. CAUSARI_BINARY runs an existing binary without touching the network;
#   6. the binary's exit code passes through unchanged;
#   7. an unsupported platform is named clearly.
#
# Standard tools only: node, python3 (for the local mirror), bash.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PKG="$ROOT/packaging/npm"
VER="$(node -p "require('$PKG/package.json').version")"
TARGET="$(node -p "require('$PKG/lib/shim.js').target()")"
ASSET="causari-v${VER}-${TARGET}.tar.gz"

WORK="$(mktemp -d)"
SERVER_PID=""
cleanup() { [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true; rm -rf "$WORK"; }
trap cleanup EXIT

pass() { printf 'ok   %s\n' "$*"; }
fail() { printf 'FAIL %s\n' "$*" >&2; exit 1; }

export XDG_CACHE_HOME="$WORK/cache"
unset CAUSARI_BINARY CAUSARI_VERSION CAUSARI_DOWNLOAD_BASE

# The commit that bumps package.json to the next version is pushed before
# its tag exists, so on that commit the release named by package.json is not
# published yet. Run the same launcher against the newest published release
# through CAUSARI_VERSION and say so; every other check is unchanged.
RELEASES="https://github.com/croviatrust/causari/releases"
if ! curl -fsSLI --retry 3 "$RELEASES/download/v$VER/SHA256SUMS.txt" >/dev/null 2>&1; then
  latest="$(curl -fsSL --retry 3 -o /dev/null -w '%{url_effective}' "$RELEASES/latest")"
  latest="${latest##*/v}"
  case "$latest" in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *) fail "release v$VER is not published and the latest release could not be resolved ($latest)" ;;
  esac
  echo "note: release v$VER is not published yet; testing the launcher against v$latest via CAUSARI_VERSION"
  export CAUSARI_VERSION="$latest"
  VER="$latest"
  ASSET="causari-v${VER}-${TARGET}.tar.gz"
fi
BIN="$XDG_CACHE_HOME/causari/$VER/$TARGET/re"

# 1. first run downloads, verifies, runs ----------------------------------
if ! out="$(node "$PKG/bin/re.js" --version 2>"$WORK/err1")"; then
  fail "first run failed: $(cat "$WORK/err1")"
fi
grep -q "first run: downloading causari v$VER" "$WORK/err1" || fail "no download message: $(cat "$WORK/err1")"
grep -q "sha256 verified" "$WORK/err1" || fail "no verification message: $(cat "$WORK/err1")"
# The release binary reports its own name, `causari`, under both file names.
[ "$out" = "causari $VER" ] || fail "expected 'causari $VER', got '$out'"
[ -x "$BIN" ] || fail "binary not cached at $BIN"
[ -x "$(dirname "$BIN")/causari" ] || fail "causari not cached next to re"
mkdir -p "$WORK/own" && cp "$BIN" "$WORK/own/re"   # kept for steps 5 and 6
pass "first run downloaded, verified and printed '$out'"

# 2. second run is a cache hit ---------------------------------------------
out="$(node "$PKG/bin/causari.js" --version 2>"$WORK/err2")"
[ "$out" = "causari $VER" ] || fail "cache hit printed '$out'"
[ ! -s "$WORK/err2" ] || fail "cache hit wrote to stderr: $(cat "$WORK/err2")"
pass "second run used the cache"

# local mirror for the failure cases ----------------------------------------
MIRROR="$WORK/mirror/v$VER"
mkdir -p "$MIRROR"
PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')"
python3 -m http.server "$PORT" --bind 127.0.0.1 -d "$WORK/mirror" >/dev/null 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 50); do
  if python3 -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:$PORT/', timeout=1)" 2>/dev/null; then break; fi
  sleep 0.1
done
export CAUSARI_DOWNLOAD_BASE="http://127.0.0.1:$PORT"

# 3. tampered archive: bytes differ from the published sum -----------------
printf 'not the release archive' > "$MIRROR/$ASSET"
printf '%s  %s\n' "$(printf 'the bytes the maintainer published' | sha256sum | cut -d' ' -f1)" "$ASSET" > "$MIRROR/SHA256SUMS.txt"
rm -rf "$XDG_CACHE_HOME"
if node "$PKG/bin/re.js" --version >"$WORK/out3" 2>"$WORK/err3"; then fail "tampered archive was accepted"; fi
grep -q "sha256 mismatch" "$WORK/err3" || fail "expected 'sha256 mismatch', got: $(cat "$WORK/err3")"
[ ! -e "$BIN" ] || fail "tampered archive left a binary in the cache"
pass "tampered archive refused, nothing cached"

# 4. sums file without the asset -------------------------------------------
printf '%s  %s\n' "$(sha256sum "$MIRROR/$ASSET" | cut -d' ' -f1)" "some-other-file.tar.gz" > "$MIRROR/SHA256SUMS.txt"
if node "$PKG/bin/re.js" --version >/dev/null 2>"$WORK/err4"; then fail "missing checksum was accepted"; fi
grep -q "no checksum for $ASSET" "$WORK/err4" || fail "expected 'no checksum', got: $(cat "$WORK/err4")"
pass "asset absent from SHA256SUMS.txt refused"

# 5. CAUSARI_BINARY: no network at all -------------------------------------
export CAUSARI_DOWNLOAD_BASE="http://127.0.0.1:9"   # nothing listens here
rm -rf "$XDG_CACHE_HOME"
out="$(CAUSARI_BINARY="$WORK/own/re" node "$PKG/bin/re.js" --version 2>"$WORK/err5")"
[ "$out" = "causari $VER" ] || fail "CAUSARI_BINARY run printed '$out': $(cat "$WORK/err5")"
[ ! -e "$XDG_CACHE_HOME" ] || fail "CAUSARI_BINARY run touched the cache"
if CAUSARI_BINARY="$WORK/does-not-exist" node "$PKG/bin/re.js" --version >/dev/null 2>"$WORK/err5b"; then fail "missing CAUSARI_BINARY accepted"; fi
grep -q "does not exist" "$WORK/err5b" || fail "expected 'does not exist', got: $(cat "$WORK/err5b")"
pass "CAUSARI_BINARY honoured, missing path named"

# 6. exit code passthrough --------------------------------------------------
set +e
"$WORK/own/re" no-such-subcommand >/dev/null 2>&1; want=$?
CAUSARI_BINARY="$WORK/own/re" node "$PKG/bin/re.js" no-such-subcommand >/dev/null 2>&1; got=$?
set -e
[ "$want" -ne 0 ] || fail "binary accepted an unknown subcommand"
[ "$want" -eq "$got" ] || fail "exit code $got, binary returned $want"
pass "exit code $got passed through"

# 7. unsupported platform ---------------------------------------------------
msg="$(node -e "try { require('$PKG/lib/shim.js').target('freebsd','x64') } catch (e) { console.log(e.message) }")"
case "$msg" in
  *"no prebuilt binary for freebsd/x64"*"cargo install causari"*) pass "unsupported platform named, source build suggested" ;;
  *) fail "unsupported platform message: $msg" ;;
esac

echo "all npm shim checks passed (v$VER, $TARGET)"
