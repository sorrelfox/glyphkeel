#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The Windows command line under Wine: a short session in the default
# workspace, attachments, export and import into a second workspace made
# through the data and settings folder overrides, a workspace by folder path,
# concurrent writers and the MCP server over standard input and output.
#
# Usage: wine-smoke.sh PATH/TO/glyphkeel.exe
# EXPECTED_VERSION, when set, is the version --version must print.

. "$(dirname "$0")/wine-lib.sh"

exe=$1
[ -f "$exe" ] || { echo "no program at $exe" >&2; exit 1; }
new_prefix smoke
log="$GK_WINE_ROOT/smoke.log"
: >"$log"
echo "(details in $log)"

# The command line; its errors go to the log.
gk() {
  wine "$exe" "$@" 2>>"$log"
}

# The command line, its output in the log too.
gk_ok() {
  gk "$@" >>"$log"
}

# gk_has TEXT ARGS...: the output of the command line contains TEXT.
gk_has() {
  text=$1
  shift
  gk "$@" | tee -a "$log" | grep -qF -- "$text"
}

local_app_data=$(unix_path "$(win_env LOCALAPPDATA)")
work_win='C:\gk-smoke'
work=$(unix_path "$work_win")
mkdir -p "$work"

echo "== the program"
version=$(gk --version | tr -d '\r')
echo "$version"
if [ -n "${EXPECTED_VERSION:-}" ]; then
  check "--version prints $EXPECTED_VERSION" [ "$version" = "glyphkeel $EXPECTED_VERSION" ]
else
  check "--version prints a version" [ -n "$version" ]
fi
check "completions powershell" gk_has "Register-ArgumentCompleter" completions powershell

echo "== a session in the default workspace"
has_database() {
  find "$1" -name workspace.db 2>/dev/null | grep -q .
}
check "workspace creates Personal" gk_ok workspace
check "the default workspace is in %LOCALAPPDATA%\\Glyphkeel" has_database "$local_app_data/Glyphkeel"
check "project create" gk_ok project create --name Smoke --key SM
check "task create" gk_ok task create --project SM --title "Check the Windows build"
check "task create, non-ASCII title" gk_ok task create --project SM --title "Umlaute äöü 日本"
check "task show --json" gk_has '"SM-1"' --json task show SM-1
check "the non-ASCII title comes back intact" gk_has "Umlaute äöü 日本" --json task show SM-2
check "task list" gk_has "SM-2" task list --project SM
check "comment add" gk_ok comment add SM-1 --body "Hello from Wine"
check "search finds the task" gk_has "SM-1" search Windows
check "time log" gk_ok time log SM-1 --duration 30m
check "backup create" gk_ok backup create
check "backup list" gk_ok backup list

echo "== attachments, export and import"
printf 'attached from Wine\n' >"$work/note.txt"
check "file attach" gk_ok file attach --task SM-1 --path "$work_win\\note.txt"
check "export --path" gk_ok export --path "$work_win\\export.zip"
check "the export archive is written" [ -s "$work/export.zip" ]
# A second workspace in folders of our own, through the overrides.
second() {
  GLYPHKEEL_DATA_DIR="$work_win\\data2" GLYPHKEEL_CONFIG_DIR="$work_win\\config2" gk "$@"
}
second_has() {
  text=$1
  shift
  second "$@" | tee -a "$log" | grep -qF -- "$text"
}
check "import into a workspace in the override folders" second_has "SM" import "$work_win\\export.zip"
check "the second workspace is in the data override" has_database "$work/data2"
check "the default workspace is left alone" has_database "$local_app_data/Glyphkeel"
check "the imported task is there" second_has "Check the Windows build" --json task show SM-1
check "the imported attachment is stored" sh -c 'grep -rlq "attached from Wine" "$1/data2"' sh "$work"

echo "== a workspace by folder path"
# The default workspace's folder, as a Windows path.
personal=$(dirname "$(find "$local_app_data/Glyphkeel" -name workspace.db | head -n 1)")
personal_win="$(win_env LOCALAPPDATA)${personal#"$local_app_data"}"
personal_win=$(printf '%s' "$personal_win" | tr '/' '\\')
echo "default workspace: $personal_win"
check "-w with the default workspace's folder" gk_has "SM" -w "$personal_win" project list
# A folder that holds no workspace is refused (not found), never created.
mkdir -p "$work/empty"
refused_empty() {
  rc=0
  gk -w "$work_win\\empty" workspace >>"$log" || rc=$?
  [ "$rc" -eq 2 ] && ! has_database "$work/empty"
}
check "-w with an empty folder is refused" refused_empty

echo "== concurrent writers"
for writer in 1 2 3 4; do
  (
    for n in 1 2 3 4 5; do
      gk -q task create --project SM --title "writer $writer task $n" || echo "writer $writer failed at $n" >>"$log"
    done
  ) &
done
wait
count=$(gk --json task list --project SM | grep -c '"key": *"SM-' || true)
echo "tasks in SM: $count"
check "4 writers x 5 tasks all stored" [ "$count" -ge 22 ]

echo "== MCP over standard input and output"
mcp_out="$GK_WINE_ROOT/mcp.out"
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_task","arguments":{"project":"SM","title":"Made over MCP äöü"}}}' \
  | timeout 120 wine "$exe" mcp >"$mcp_out" 2>>"$log" || true
head -c 300 "$mcp_out"
echo
answer() {
  grep "\"id\":$1[,}]" "$mcp_out" | grep -q -- "$2"
}
check "mcp initialize" answer 1 '"serverInfo"'
check "mcp tools/list" answer 2 '"create_task"'
check "mcp tools/call create_task" answer 3 '"result"'
check "the task made over MCP is in the workspace" gk_has "Made over MCP äöü" task list --project SM

finish
