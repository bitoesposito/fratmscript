#!/bin/sh
# Installs the fratm CLI from the latest GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/kkratterf/fratmscript/main/install.sh | sh
#
# Environment:
#   FRATM_VERSION      release to install, e.g. v0.5.0 (default: latest)
#   FRATM_INSTALL_DIR  where to put the binary (default: $HOME/.local/bin)

set -eu

REPO="kkratterf/fratmscript"
INSTALL_DIR="${FRATM_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
fail() { printf 'fratm: %s\n' "$*" >&2; exit 1; }

command -v curl >/dev/null 2>&1 || fail "serve curl pe scaricà fratm (curl is required)"
command -v tar >/dev/null 2>&1 || fail "serve tar (tar is required)"

os="$(uname -s)"
arch="$(uname -m)"
case "$os-$arch" in
  Linux-x86_64 | Linux-amd64) target="x86_64-unknown-linux-gnu" ;;
  Darwin-arm64 | Darwin-aarch64) target="aarch64-apple-darwin" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  *)
    fail "nun ce sta ancora nu binario pe $os $arch. Compilalo tu:
  cargo install --git https://github.com/$REPO fratm-cli"
    ;;
esac

version="${FRATM_VERSION:-}"
if [ -z "$version" ]; then
  # /releases/latest redirects to /releases/tag/<tag>; no API token needed
  latest="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")" \
    || fail "nun riesco a truvà l'ultima release"
  version="${latest##*/}"
fi
case "$version" in
  v*) ;;
  *) version="v$version" ;;
esac

name="fratm-$version-$target"
base="https://github.com/$REPO/releases/download/$version"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "Scarico fratm $version ($target)..."
curl -fsSL "$base/$name.tar.gz" -o "$tmp/$name.tar.gz" || fail "download fallito: $base/$name.tar.gz"

if curl -fsSL "$base/SHA256SUMS.txt" -o "$tmp/SHA256SUMS.txt" 2>/dev/null; then
  expected="$(grep " $name.tar.gz\$" "$tmp/SHA256SUMS.txt" | cut -d' ' -f1)"
  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$tmp/$name.tar.gz" | cut -d' ' -f1)"
  else
    actual="$(shasum -a 256 "$tmp/$name.tar.gz" | cut -d' ' -f1)"
  fi
  [ -n "$expected" ] && [ "$expected" = "$actual" ] || fail "checksum sbagliato pe $name.tar.gz"
fi

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$INSTALL_DIR"
cp "$tmp/$name/fratm" "$INSTALL_DIR/fratm"
chmod +x "$INSTALL_DIR/fratm"

say "Tutto appost! 🤌 fratm installato in $INSTALL_DIR/fratm"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) say "Aggiungi $INSTALL_DIR al PATH, per esempio: export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac
