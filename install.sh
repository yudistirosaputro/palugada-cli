#!/bin/sh
# palugada installer — downloads the latest prebuilt release for your platform
# and installs it, keeping the bundled knowledge/ profiles next to the binary.
#
#   curl -fsSL https://raw.githubusercontent.com/yudistirosaputro/palugada-cli/main/install.sh | sh
#
# Overridable: PALUGADA_INSTALL_DIR (default ~/.local/share/palugada),
#              PALUGADA_BIN_DIR     (default ~/.local/bin).
set -eu

REPO="yudistirosaputro/palugada-cli"
INSTALL_DIR="${PALUGADA_INSTALL_DIR:-$HOME/.local/share/palugada}"
BIN_DIR="${PALUGADA_BIN_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)
    case "$arch" in
      x86_64 | amd64) target="x86_64-unknown-linux-gnu" ;;
      *) echo "No prebuilt Linux binary for '$arch' (only x86_64). Build from source: https://github.com/$REPO" >&2; exit 1 ;;
    esac
    ;;
  Darwin)
    case "$arch" in
      arm64 | aarch64) target="aarch64-apple-darwin" ;;
      x86_64) target="x86_64-apple-darwin" ;;
      *) echo "No prebuilt macOS binary for '$arch'." >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS '$os'. On Windows, download the .zip from https://github.com/$REPO/releases/latest" >&2
    exit 1
    ;;
esac

asset="palugada-${target}.tar.gz"
url="https://github.com/${REPO}/releases/latest/download/${asset}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading ${asset} ..."
if command -v curl >/dev/null 2>&1; then
  curl -fSL "$url" -o "$tmp/$asset"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$tmp/$asset" "$url"
else
  echo "Need curl or wget to download." >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
tar xzf "$tmp/$asset" -C "$INSTALL_DIR"

mkdir -p "$BIN_DIR"
ln -sf "$INSTALL_DIR/palugada" "$BIN_DIR/palugada"

echo "Installed palugada -> $BIN_DIR/palugada"
echo "Bundled knowledge/ profiles live in $INSTALL_DIR (keep them next to the binary)."

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "Note: $BIN_DIR is not on your PATH. Add it, e.g.: export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac

"$BIN_DIR/palugada" --version 2>/dev/null || true
