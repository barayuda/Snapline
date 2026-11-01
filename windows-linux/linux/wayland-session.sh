#!/usr/bin/env bash
# Runs CMD inside a headless Wayland session (sway, with XWayland) whose screen is solid red, for tests.
# Usage: linux/wayland-session.sh CMD...      (run it under dbus-run-session: the portals need a session bus)
set -euo pipefail
run=$(mktemp -d)
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-$run}
printf 'output * bg #ff0000 solid_color\nxwayland force\nexec sh -c '"'"'echo "$DISPLAY" > %s/display'"'"'\n' "$run" >"$run/config"
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman sway -c "$run/config" >"$run/sway.log" 2>&1 &
sway=$!
trap 'kill $sway 2>/dev/null; rm -rf "$run"' EXIT
for _ in $(seq 100); do [ -s "$run/display" ] && break; sleep 0.1; done
[ -s "$run/display" ] || { echo "sway did not start:"; cat "$run/sway.log"; exit 1; }
WAYLAND_DISPLAY=$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' -printf '%f\n' | head -1)
DISPLAY=$(cat "$run/display")
export WAYLAND_DISPLAY DISPLAY XDG_SESSION_TYPE=wayland
sleep 1 # let swaybg draw the background
"$@"
