# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Shared set-up for the checks of the Windows build under Wine, sourced by
# the wine-*.sh scripts in this folder. They run in the windows-cross CI
# image (`cargo xtask ci --distro windows-cross`), without a display.
#
# GK_WINE_ROOT   the folder for the Wine prefixes (default: /tmp/gk-wine)
# GK_WINE_STRICT 1 turns the checks of what packaging adds later (install
#                folder, PATH, the glyphkeel:// handler, the command line
#                next to the app) from skipped into failed

set -eu

: "${GK_WINE_ROOT:=${TMPDIR:-/tmp}/gk-wine}"
: "${GK_WINE_STRICT:=0}"
# No window can open: no display, and no Mono or Gecko install prompts.
export WINEDEBUG=-all DISPLAY= WAYLAND_DISPLAY= WINEDLLOVERRIDES="mscoree,mshtml="
# Wine converts the command line with the locale's encoding; UTF-8 keeps
# non-ASCII titles intact.
export LANG=C.UTF-8 NO_COLOR=1

passed=0
failed=0
skipped=0

pass() {
  passed=$((passed + 1))
  echo "PASS: $*"
}

fail() {
  failed=$((failed + 1))
  echo "FAIL: $*"
}

skip() {
  skipped=$((skipped + 1))
  echo "SKIP: $*"
}

# check DESCRIPTION COMMAND...: passes when the command succeeds.
check() {
  what=$1
  shift
  if "$@"; then pass "$what"; else fail "$what"; fi
}

# pending DESCRIPTION REASON: something the packaging doesn't provide yet.
# Skipped with the reason, or failed with GK_WINE_STRICT=1.
pending() {
  if [ "$GK_WINE_STRICT" = 1 ]; then
    fail "$1 ($2)"
  else
    skip "$1: not there yet, $2"
  fi
}

# new_prefix NAME: a new, empty Wine prefix, booted, in WINEPREFIX.
new_prefix() {
  WINEPREFIX="$GK_WINE_ROOT/$1"
  export WINEPREFIX
  rm -rf "$WINEPREFIX"
  mkdir -p "$GK_WINE_ROOT"
  wineboot -i >/dev/null 2>&1
  wineserver -w
}

# Stops this prefix's Wine server (and nothing else).
stop_wine() {
  wineserver -k 2>/dev/null || true
}

# A Windows environment variable of the prefix, e.g. win_env LOCALAPPDATA.
win_env() {
  wine cmd /c echo "%$1%" 2>/dev/null | tr -d '\r'
}

# The Linux path of a Windows path.
unix_path() {
  winepath -u "$1" 2>/dev/null | tr -d '\r'
}

# Prints the values of a registry key, without carriage returns; empty when
# the key is missing.
reg_query() {
  wine reg query "$@" 2>/dev/null | tr -d '\r' || true
}

# Prints the totals, and the end of the script's log (in $log, if set) when a
# check failed; fails when a check failed.
finish() {
  if [ "$failed" -ne 0 ] && [ -n "${log:-}" ] && [ -s "$log" ]; then
    echo "== the last lines of $log"
    tail -n 40 "$log"
  fi
  echo "== $passed passed, $failed failed, $skipped skipped"
  stop_wine
  [ "$failed" -eq 0 ]
}
