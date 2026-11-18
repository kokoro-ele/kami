#!/bin/sh
# Install Kami (紙) on macOS from the latest GitHub release.
#   curl -fsSL https://raw.githubusercontent.com/kokoro-ele/kami/main/install.sh | sh
# Files fetched with curl aren't quarantined, so macOS opens Kami without the
# "can't verify" prompt. Set KAMI_CLI=1 to also install the `kami` CLI to ~/.local/bin.
set -eu

REPO="kokoro-ele/kami"
say() { printf '\033[1m%s\033[0m\n' "$*"; }
die() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || die "the desktop app is macOS-only for now. For the CLI: cargo install --git https://github.com/$REPO kami-cli"
case "$(uname -m)" in
  arm64) arch="aarch64" ;;
  x86_64) arch="x86_64" ;;
  *) die "unsupported CPU: $(uname -m)" ;;
esac

say "Looking up the latest release…"
assets=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep -o '"browser_download_url": *"[^"]*"' | sed 's/.*"\(https[^"]*\)"/\1/')
dmg=$(printf '%s\n' "$assets" | grep "_${arch}\.dmg$" | head -n1 || true)
[ -n "$dmg" ] || dmg=$(printf '%s\n' "$assets" | grep "_universal\.dmg$" | head -n1 || true)
[ -n "$dmg" ] || die "no Kami build for $arch in the latest release yet — see https://github.com/$REPO/releases"

tmp=$(mktemp -d)
mnt="$tmp/mnt"
cleanup() { hdiutil detach "$mnt" -quiet 2>/dev/null || true; rm -rf "$tmp"; }
trap cleanup EXIT INT TERM

say "Downloading $(basename "$dmg")…"
curl -fL --progress-bar -o "$tmp/kami.dmg" "$dmg"

mkdir -p "$mnt"
hdiutil attach "$tmp/kami.dmg" -nobrowse -readonly -mountpoint "$mnt" -quiet
[ -d "$mnt/Kami.app" ] || die "Kami.app not found in the disk image"

dest="/Applications"
[ -w "$dest" ] || dest="$HOME/Applications"
mkdir -p "$dest"
osascript -e 'quit app "Kami"' >/dev/null 2>&1 || true
rm -rf "$dest/Kami.app"
cp -R "$mnt/Kami.app" "$dest/"
xattr -cr "$dest/Kami.app" 2>/dev/null || true
say "Installed $dest/Kami.app"

if [ "${KAMI_CLI:-0}" = "1" ]; then
  cli=$(printf '%s\n' "$assets" | grep "kami-cli-${arch}-apple-darwin\.tar\.gz$" | head -n1 || true)
  if [ -n "$cli" ]; then
    mkdir -p "$HOME/.local/bin"
    curl -fsSL "$cli" | tar xz -C "$HOME/.local/bin" kami
    say "Installed ~/.local/bin/kami"
  else
    printf 'No prebuilt CLI for %s; use: cargo install --git https://github.com/%s kami-cli\n' "$arch" "$REPO"
  fi
fi

open "$dest/Kami.app"
