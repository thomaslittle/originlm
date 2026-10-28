#!/usr/bin/env bash
# Headless only. Run from the repository root after building originlm-desktop.
set -euo pipefail
mkdir -p docs/review/screens/desktop .verification
export XDG_SESSION_TYPE=x11
export LAI_DESKTOP_STATE_DIR="$PWD/.verification/desktop-example-state"
export LAI_DESKTOP_WINDOW=1440x920
capture() {
  export LAI_DESKTOP_PAGE="$1"
  export ORIGINLM_CAPTURE_DEST="$PWD/docs/review/screens/desktop/$2.png"
  env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET xvfb-run -a -s '-screen 0 1920x1080x24' bash -c '
    ./target/debug/originlm-desktop --example > .verification/native-example.log 2>&1 &
    capture_pid=$!
    trap '\''kill "$capture_pid" 2>/dev/null || true'\'' EXIT
    sleep 8
    import -window OriginLM "$ORIGINLM_CAPTURE_DEST"
  '
}
unset LAI_DESKTOP_MINI_VIEW
capture Overview overview-costs
capture Settings settings
export LAI_DESKTOP_MINI_VIEW=1
capture Overview mini-view
