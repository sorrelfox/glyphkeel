#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The installed MCP server: `glyphkeel mcp` over standard input and output
# answers initialize, lists its tools and makes a task. Runs after smoke.sh,
# whose project SM it uses.

. /ci/lib.sh

echo "== MCP over standard input and output"
out=/out/mcp.out
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_task","arguments":{"project":"SM","title":"Made over MCP"}}}' \
  | timeout 120 glyphkeel mcp >"$out" 2>/out/mcp.err || true
head -c 300 "$out"
echo
answer() {
  grep "\"id\":$1[,}]" "$out" | grep -q -- "$2"
}
answer 1 '"serverInfo"' || fail "mcp initialize has no serverInfo"
answer 1 '"protocolVersion"' || fail "mcp initialize has no protocolVersion"
answer 2 '"create_task"' || fail "mcp tools/list has no create_task"
answer 3 '"result"' || fail "mcp tools/call create_task failed"
glyphkeel task list --project SM | grep -q 'Made over MCP' || fail "the task made over MCP is not listed"
echo "mcp: passed"
