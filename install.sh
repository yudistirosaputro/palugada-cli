#!/bin/sh
# palugada installer — downloads the latest prebuilt release for your platform
# and installs it, keeping the bundled knowledge/ profiles next to the binary.
#
#   curl -fsSL https://raw.githubusercontent.com/yudistirosaputro/palugada-cli/main/install.sh | sh
#
# Overridable: PALUGADA_INSTALL_DIR (default ~/.local/share/palugada),
#              PALUGADA_BIN_DIR     (default ~/.local/bin),
#              PALUGADA_VERSION     (default latest; pin e.g. v0.2.4),
#              PALUGADA_SKIP_CHECKSUM (set to 1 to bypass verification — unsafe).
set -eu

REPO="yudistirosaputro/palugada-cli"
INSTALL_DIR="${PALUGADA_INSTALL_DIR:-$HOME/.local/share/palugada}"
BIN_DIR="${PALUGADA_BIN_DIR:-$HOME/.local/bin}"
VERSION="${PALUGADA_VERSION:-latest}"

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
if [ "$VERSION" = "latest" ]; then
  base_url="https://github.com/${REPO}/releases/latest/download"
else
  base_url="https://github.com/${REPO}/releases/download/${VERSION}"
fi

# Download $1 (URL) → $2 (path). Fails the script (set -e) on HTTP/transport error.
dl() {
  if command -v curl >/dev/null 2>&1; then
    curl -fSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" "$1"
  else
    echo "Need curl or wget to download." >&2
    exit 1
  fi
}

# Verify $1 (file) against $2 (a `shasum -a 256`-style sidecar). Hard-fail on
# mismatch; warn only when no checksum tool is available (can't verify).
verify_sha256() {
  _file="$1"; _sumfile="$2"
  _expected="$(awk '{print $1}' "$_sumfile" | head -1)"
  if command -v sha256sum >/dev/null 2>&1; then
    _actual="$(sha256sum "$_file" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    _actual="$(shasum -a 256 "$_file" | awk '{print $1}')"
  else
    echo "Warning: no sha256sum/shasum found — cannot verify download integrity." >&2
    return 0
  fi
  if [ -z "$_expected" ] || [ "$_expected" != "$_actual" ]; then
    echo "ERROR: checksum mismatch for $_file" >&2
    echo "  expected: $_expected" >&2
    echo "  actual:   $_actual" >&2
    exit 1
  fi
  echo "Checksum verified ($_actual)"
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading ${asset} (${VERSION}) ..."
dl "$base_url/$asset" "$tmp/$asset"

if [ "${PALUGADA_SKIP_CHECKSUM:-0}" = "1" ]; then
  echo "Warning: PALUGADA_SKIP_CHECKSUM=1 — skipping integrity verification." >&2
else
  echo "Verifying checksum ..."
  dl "$base_url/$asset.sha256" "$tmp/$asset.sha256"
  verify_sha256 "$tmp/$asset" "$tmp/$asset.sha256"
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
