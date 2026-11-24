#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The never-online check. The container has no network but loopback
# (`--network none`). The desktop app, the command line and the MCP server
# run under strace, which follows every process they start, WebKitGTK's web
# and network processes included, and records every connect, bind and send
# with an address. Any IPv4 or IPv6 address fails the check; local sockets
# and D-Bus are allowed.

. /ci/lib.sh

glyphkeel import /data/S >/out/import-S.log
# The hostile set: remote images, links, frames and forms in Markdown.
glyphkeel import /data/hostile >/out/import-hostile.log 2>&1 \
  || fail "the hostile data set does not import (see import-hostile.log)"

start_display
status=0
strace -f -qq -s 200 -e trace=execve,connect,bind,sendto,sendmsg,sendmmsg \
  -o /out/trace.log dbus-run-session -- sh /ci/offline-session.sh || status=$?
stop_display
[ "$status" -eq 0 ] || fail "the session failed (exit $status); see app.log"

python3 -I /ci/connect_check.py /out/trace.log \
  --require glyphkeel-app --require glyphkeel \
  --require WebKitWebProcess --require WebKitNetworkProcess
