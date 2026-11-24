# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Shared set-up for the checks of the installed packages, sourced by the
# scripts in this folder. They run in a clean container of a distribution
# with the deb or rpm installed (`cargo xtask ci --suite packages,offline,perf`).
#
# Folders inside the container:
#   /ci    this folder, read-only
#   /data  the data sets S, L and hostile from the seed tool, read-only
#   /out   results and logs, kept on the host

set -eu

export HOME=/tmp/home
export XDG_DATA_HOME="$HOME/.local/share"
export XDG_CONFIG_HOME="$HOME/.config"
export XDG_STATE_HOME="$HOME/.local/state"
export XDG_CACHE_HOME="$HOME/.cache"
export XDG_RUNTIME_DIR=/tmp/runtime
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR" /out
chmod 700 "$XDG_RUNTIME_DIR"
# A real locale, as on a desktop. The app passes it to the page; under C the
# page would fall back to the web view's own language tag.
export LANG=en_US.UTF-8
export NO_COLOR=1

fail() {
  echo "FAILED: $*" >&2
  exit 1
}

# A headless X server on display :99, listening on its local socket only.
# The app runs on X11 there; the PID is in XVFB_PID.
start_display() {
  Xvfb :99 -screen 0 1600x1000x24 -nolisten tcp -nolisten local >/out/xvfb.log 2>&1 &
  XVFB_PID=$!
  export DISPLAY=:99 GDK_BACKEND=x11
  i=0
  while [ ! -S /tmp/.X11-unix/X99 ]; do
    i=$((i + 1))
    [ "$i" -le 100 ] || fail "Xvfb did not start (see xvfb.log)"
    sleep 0.1
  done
}

stop_display() {
  kill "$XVFB_PID" 2>/dev/null || true
  wait "$XVFB_PID" 2>/dev/null || true
}

# Waits until a visible window whose name contains $1 exists, up to $2
# seconds.
wait_for_window() {
  i=0
  while ! xdotool search --onlyvisible --name "$1" >/dev/null 2>&1; do
    i=$((i + 1))
    [ "$i" -le $(($2 * 10)) ] || return 1
    sleep 0.1
  done
}

# The keys of the workspace's projects, or of a project's tasks ($1), one per
# line, from the command line's JSON output.
project_keys() {
  glyphkeel --json project list | python3 -I -c '
import json, sys
for item in json.load(sys.stdin)["items"]:
    print(item["key"])'
}

task_keys() {
  glyphkeel --json task list --project "$1" | python3 -I -c '
import json, sys
for item in json.load(sys.stdin)["items"]:
    print(item["key"])'
}
