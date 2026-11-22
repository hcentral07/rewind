#!/bin/sh
# Installs the Rewind command-line tool on Linux.
#   curl -fsSL https://raw.githubusercontent.com/hcentral07/rewind/main/scripts/install.sh | sh
# Options (environment): REWIND_VERSION=v0.3.0   REWIND_BIN_DIR=$HOME/.local/bin
set -eu

REPO="hcentral07/rewind"
BIN_DIR="${REWIND_BIN_DIR:-$HOME/.local/bin}"

[ "$(uname -s)" = "Linux" ] || { echo "This installer is for Linux. Windows: https://github.com/$REPO/releases" >&2; exit 1; }
case "$(uname -m)" in
  x86_64|amd64) ASSET="rewind-linux-x86_64.tar.gz" ;;
  *) echo "No prebuilt binary for $(uname -m) yet. Build from source: https://github.com/$REPO#build-from-source" >&2; exit 1 ;;
esac

command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "sha256sum is required" >&2; exit 1; }

TAG="${REWIND_VERSION:-}"
if [ -z "$TAG" ]; then
  TAG="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
fi
[ -n "$TAG" ] || { echo "Could not find the latest release. Is there one yet? https://github.com/$REPO/releases" >&2; exit 1; }

BASE="https://github.com/$REPO/releases/download/$TAG"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading Rewind $TAG ..."
curl -fsSL "$BASE/$ASSET" -o "$TMP/$ASSET"
curl -fsSL "$BASE/$ASSET.sha256" -o "$TMP/$ASSET.sha256"

echo "Verifying checksum ..."
( cd "$TMP" && sha256sum -c "$ASSET.sha256" )

tar -xzf "$TMP/$ASSET" -C "$TMP"
mkdir -p "$BIN_DIR"
install -m 755 "$TMP/rewind" "$BIN_DIR/rewind"

echo "Installed: $BIN_DIR/rewind"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "Add $BIN_DIR to your PATH, e.g.:  echo 'export PATH=\"$BIN_DIR:\$PATH\"' >> ~/.profile" ;;
esac
echo "Try:  rewind watch ~/Documents     (and: rewind --help)"
