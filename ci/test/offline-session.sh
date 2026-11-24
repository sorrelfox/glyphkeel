#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# What the never-online check does while it traces (see offline.sh): the app
# shows a board, another program changes it, task windows show hostile
# Markdown, and an MCP client reads and writes.

. /ci/lib.sh

project=$(project_keys | grep -vx HOST | head -n 1)
first=$(task_keys "$project" | head -n 1)
glyphkeel-app "glyphkeel://open/$first" >/out/app.log 2>&1 &
app=$!
wait_for_window Glyphkeel 60 || fail "the app shows no window"
sleep 3

# A change from another program reaches the open board.
glyphkeel task create --project "$project" --title "Made while offline" >/dev/null

# Task windows with the hostile descriptions and comments.
for key in $(task_keys HOST | head -n 30); do
  glyphkeel open "$key"
  sleep 0.3
done
sleep 5

python3 -I /ci/gk.py mcp-probe "$first"
sleep 2

kill "$app"
wait "$app" || true

# The page must have drawn its first screen, or nothing was exercised.
if grep -q 'did not signal its first screen' /out/app.log; then
  fail "the page never drew its first screen (see app.log)"
fi
