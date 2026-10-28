#!/bin/sh
# Remove the OriginLM helper service, tray autostart, browser Native Messaging
# manifests and the binary. User data is kept unless --purge is passed.
#
#   sh scripts/service/uninstall.sh
#   sh scripts/service/uninstall.sh --purge
set -eu

PURGE=0
for arg in "$@"; do
  case "$arg" in
    --purge) PURGE=1 ;;
    *) printf 'error: unknown option %s\n' "$arg" >&2; exit 2 ;;
  esac
done

BIN_DIR="${LAI_BIN_DIR:-$HOME/.local/bin}"
STATE_DIR="${LAI_STATE_DIR:-$HOME/.originlm}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}"
case "${LOCAL_AI_KIT_STATE_DIR:-}" in
  /*) default_data="$LOCAL_AI_KIT_STATE_DIR" ;;
  *)
    if [ "$(uname -s)" = Darwin ]; then
      default_data="$HOME/Library/Application Support/originlm"
    else
      case "${XDG_STATE_HOME:-}" in
        /*) default_data="$XDG_STATE_HOME/originlm" ;;
        *) default_data="$HOME/.local/state/originlm" ;;
      esac
    fi ;;
esac
DATA_DIR="${LAI_DATA_DIR:-$default_data}"
for path in "$BIN_DIR" "$STATE_DIR" "$DATA_DIR" "$CONFIG_DIR"; do
  case "$path" in /*) ;; *) printf 'error: uninstall paths must be absolute\n' >&2; exit 2 ;; esac
  case "$path" in *'
'*) printf 'error: uninstall paths must not contain newlines\n' >&2; exit 2 ;; esac
done
if [ "$PURGE" -eq 1 ]; then
  for path in "$STATE_DIR" "$DATA_DIR"; do
    canonical="$path"
    if [ -d "$path" ]; then canonical="$(CDPATH= cd -- "$path" && pwd -P)"; fi
    home_canonical="$(CDPATH= cd -- "$HOME" && pwd -P)"
    case "$canonical" in /|"$home_canonical") printf 'error: refusing to purge the filesystem root or home directory\n' >&2; exit 2 ;; esac
  done
fi

say() { printf '%s\n' "$*"; }

if [ "$(uname -s)" = Darwin ]; then
  plist="$HOME/Library/LaunchAgents/dev.originlm.helper.plist"
  launchctl unload "$plist" >/dev/null 2>&1 || true
  rm -f "$plist"
  rm -f "$HOME/Library/LaunchAgents/dev.originlm.tray.plist"
  say "Removed the launchd agent"
else
  systemctl --user disable --now originlm-helper.service >/dev/null 2>&1 || true
  rm -f "$CONFIG_DIR/systemd/user/originlm-helper.service"
  systemctl --user daemon-reload >/dev/null 2>&1 || true
  rm -f "$CONFIG_DIR/autostart/originlm-tray.desktop"
  say "Removed the systemd user unit"
fi

# Native Messaging manifests for every browser we may have registered.
for path in \
  "$CONFIG_DIR/google-chrome/NativeMessagingHosts" \
  "$CONFIG_DIR/chromium/NativeMessagingHosts" \
  "$CONFIG_DIR/microsoft-edge/NativeMessagingHosts" \
  "$CONFIG_DIR/BraveSoftware/Brave-Browser/NativeMessagingHosts" \
  "$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts" \
  "$HOME/Library/Application Support/Chromium/NativeMessagingHosts" \
  "$HOME/Library/Application Support/Microsoft Edge/NativeMessagingHosts" \
  "$HOME/Library/Application Support/Mozilla/NativeMessagingHosts" \
  "$HOME/.mozilla/native-messaging-hosts"; do
  rm -f "$path/com.originlm.host.json" "$path/dev.originlm.helper.json" 2>/dev/null || true
done

rm -f "$BIN_DIR/originlm"
if [ "$PURGE" -eq 1 ]; then
  rm -rf "$STATE_DIR" "$DATA_DIR"
  say "Removed the binary, service and user data"
else
  rm -f "$STATE_DIR/broker-token.json"
  say "Removed the binary and service; user data kept in $STATE_DIR and $DATA_DIR (use --purge to delete)"
fi
