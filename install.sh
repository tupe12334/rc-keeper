#!/bin/sh
# Install rc-keeper from GitHub Releases on macOS or Linux.
#
# Usage:
#   sh install.sh [--cli | --gui] [--dry-run]
#
# Modes:
#   --gui   desktop app (default on a desktop)
#   --cli   command-line tool only, into ~/.local/bin
#   Auto:   CLI when SSH_CONNECTION is set, or on Linux with no DISPLAY and no WAYLAND_DISPLAY.
#
# Environment:
#   RC_KEEPER_VERSION=v0.1.0   install this tag instead of the latest release
#   RC_KEEPER_API_BASE=...     override the GitHub API base URL (for tests)
set -eu

REPO="tupe12334/rc-keeper"
API_BASE="${RC_KEEPER_API_BASE:-https://api.github.com}"
DRY_RUN=0
WANT=""

die() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

info() {
  printf '==> %s\n' "$1"
}

# Copies $1 to $2 through a rename. Writing over a binary in place gets the new one
# SIGKILLed on macOS (stale code signature) and fails with "Text file busy" on Linux
# while the old one runs (the daemon).
place_bin() {
  if ! { cp "$1" "$2.new" && chmod +x "$2.new" && mv -f "$2.new" "$2"; }; then
    die "could not install $2"
  fi
}

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --cli) [ "$WANT" != gui ] || die "use either --cli or --gui, not both"; WANT=cli ;;
    --gui) [ "$WANT" != cli ] || die "use either --cli or --gui, not both"; WANT=gui ;;
    *) die "unknown option: $arg (supported: --cli, --gui, --dry-run)" ;;
  esac
done

command -v curl >/dev/null 2>&1 || die "curl is required"

case "$(uname -s)" in
  Darwin) OS=macos ;;
  Linux) OS=linux ;;
  *) die "unsupported OS: $(uname -s). On Windows, run install.ps1 instead." ;;
esac

case "$(uname -m)" in
  x86_64|amd64) ARCH=x86_64; DEB_ARCH=amd64; APPIMAGE_ARCH=amd64 ;;
  aarch64|arm64) ARCH=aarch64; DEB_ARCH=arm64; APPIMAGE_ARCH=aarch64 ;;
  *) die "unsupported CPU: $(uname -m)" ;;
esac

# Pick the mode. An explicit flag wins. Otherwise use CLI for SSH sessions and headless Linux.
if [ "$WANT" = cli ]; then
  MODE=cli
elif [ "$WANT" = gui ]; then
  MODE=gui
elif [ -n "${SSH_CONNECTION:-}" ]; then
  MODE=cli
elif [ "$OS" = linux ] && [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  MODE=cli
else
  MODE=gui
fi
info "Mode: $MODE"

if [ -n "${RC_KEEPER_VERSION:-}" ]; then
  API_URL="$API_BASE/repos/$REPO/releases/tags/$RC_KEEPER_VERSION"
else
  API_URL="$API_BASE/repos/$REPO/releases/latest"
fi

TMP="$(mktemp -d)"
MNT=""
cleanup() {
  if [ -n "$MNT" ]; then
    hdiutil detach -quiet "$MNT" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

info "Looking up release from $REPO"
status=$(curl -sS -o "$TMP/release.json" -w '%{http_code}' \
  -H 'Accept: application/vnd.github+json' "$API_URL") \
  || die "could not reach $API_URL"
case "$status" in
  200) ;;
  404)
    if [ -n "${RC_KEEPER_VERSION:-}" ]; then
      die "release $RC_KEEPER_VERSION not found. See https://github.com/$REPO/releases"
    fi
    die "no published release yet for rc-keeper. See https://github.com/$REPO/releases"
    ;;
  403) die "GitHub API refused the request (HTTP 403). It may be rate limited. Try again later." ;;
  *) die "GitHub API returned HTTP $status for $API_URL" ;;
esac

TAG=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP/release.json" | head -n 1)
URLS=$(sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP/release.json")
[ -n "$TAG" ] || die "could not read tag name from the GitHub API response"

# Pick the asset. Order matters: first match wins.
URL=""
if [ "$MODE" = cli ]; then
  if [ "$OS" = macos ]; then
    URL=$(printf '%s\n' "$URLS" | grep -E '/rc-keeper-universal-apple-darwin\.tar\.gz$' | head -n 1)
    [ -n "$URL" ] || die "release $TAG has no macOS CLI archive (rc-keeper-universal-apple-darwin.tar.gz)"
  else
    URL=$(printf '%s\n' "$URLS" | grep -E "/rc-keeper-$ARCH-unknown-linux-gnu\\.tar\\.gz\$" | head -n 1)
    [ -n "$URL" ] || die "release $TAG has no Linux CLI archive for $ARCH (rc-keeper-$ARCH-unknown-linux-gnu.tar.gz)"
  fi
elif [ "$OS" = macos ]; then
  URL=$(printf '%s\n' "$URLS" | grep -E '/[^/]*universal\.dmg$' | head -n 1)
  [ -n "$URL" ] || URL=$(printf '%s\n' "$URLS" | grep -E '\.app\.tar\.gz$' | head -n 1)
  [ -n "$URL" ] || die "release $TAG has no macOS app asset (*universal.dmg or *.app.tar.gz)"
else
  URL=$(printf '%s\n' "$URLS" | grep -E "_$APPIMAGE_ARCH\\.AppImage\$" | head -n 1)
  [ -n "$URL" ] || URL=$(printf '%s\n' "$URLS" | grep -E "_$DEB_ARCH\\.deb\$" | head -n 1)
  [ -n "$URL" ] || die "release $TAG has no Linux app asset (*_$APPIMAGE_ARCH.AppImage or *_$DEB_ARCH.deb)"
fi

info "Release: $TAG"
info "Asset: $URL"
if [ "$DRY_RUN" = 1 ]; then
  printf '%s\n' "$URL"
  exit 0
fi

FILE="$TMP/${URL##*/}"
info "Downloading"
curl -fsSL -o "$FILE" "$URL" || die "download failed: $URL"

install_cli() {
  mkdir -p "$TMP/cli"
  tar -xzf "$FILE" -C "$TMP/cli" || die "could not extract $FILE"
  BIN=$(find "$TMP/cli" -type f -name rc-keeper | head -n 1)
  [ -n "$BIN" ] || die "rc-keeper not found in $FILE"

  BIN_DIR="$HOME/.local/bin"
  mkdir -p "$BIN_DIR"
  place_bin "$BIN" "$BIN_DIR/rc-keeper"
  info "Installed $BIN_DIR/rc-keeper"
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) info "Add $BIN_DIR to your PATH to run rc-keeper." ;;
  esac

  printf '\nNext steps:\n'
  printf '  rc-keeper add <folder>     add a folder to keep running\n'
  printf '  rc-keeper service install  run the daemon at login\n'
  if [ "$OS" = linux ]; then
    printf '  loginctl enable-linger %s   keep the daemon running after you log out\n' "$(id -un)"
  fi
}

install_macos() {
  case "$FILE" in
    *.dmg)
      MNT="$TMP/mnt"
      mkdir -p "$MNT"
      info "Mounting disk image"
      hdiutil attach -nobrowse -noautoopen -quiet -mountpoint "$MNT" "$FILE" >/dev/null \
        || die "could not mount $FILE"
      SRC="$MNT/rc-keeper.app"
      ;;
    *.app.tar.gz)
      mkdir -p "$TMP/extract"
      tar -xzf "$FILE" -C "$TMP/extract" || die "could not extract $FILE"
      SRC="$TMP/extract/rc-keeper.app"
      ;;
  esac
  [ -d "$SRC" ] || die "rc-keeper.app not found in $FILE"

  if [ -w /Applications ]; then
    SUDO=""
  else
    info "/Applications needs admin rights. You may be asked for your password."
    SUDO="sudo"
  fi

  info "Copying rc-keeper.app to /Applications"
  $SUDO rm -rf /Applications/rc-keeper.app
  $SUDO cp -R "$SRC" /Applications/ || die "could not copy to /Applications"
  $SUDO xattr -dr com.apple.quarantine /Applications/rc-keeper.app 2>/dev/null || true

  info "Installed /Applications/rc-keeper.app. Opening it."
  open /Applications/rc-keeper.app
}

install_linux() {
  case "$FILE" in
    *.AppImage)
      BIN_DIR="$HOME/.local/bin"
      APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
      mkdir -p "$BIN_DIR" "$APPS_DIR"
      place_bin "$FILE" "$BIN_DIR/rc-keeper-gui"
      cat > "$APPS_DIR/rc-keeper.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=rc-keeper
Comment=Keep claude remote-control running
Exec=$BIN_DIR/rc-keeper-gui
Terminal=false
Categories=Utility;
EOF
      info "Installed $BIN_DIR/rc-keeper-gui and $APPS_DIR/rc-keeper.desktop"
      case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *) info "Add $BIN_DIR to your PATH to run rc-keeper-gui from a shell." ;;
      esac
      ;;
    *.deb)
      command -v dpkg >/dev/null 2>&1 || die "dpkg is required to install the .deb"
      if [ "$(id -u)" -eq 0 ]; then SUDO=""; else SUDO="sudo"; fi
      info "Installing $(basename "$FILE") with dpkg. You may be asked for your password."
      $SUDO dpkg -i "$FILE" || die "dpkg failed. Run: sudo apt-get -f install"
      info "Installed rc-keeper."
      ;;
  esac
}

if [ "$MODE" = cli ]; then
  install_cli
else
  case "$OS" in
    macos) install_macos ;;
    linux) install_linux ;;
  esac
fi
