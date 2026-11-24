#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The performance measurements with data set L, on the installed release
# build, under a headless X server (software rendering). The table lands in
# /out/perf.md. With PERF_ENFORCE=1 a result over its budget fails the run.

. /ci/lib.sh

start_display
status=0
dbus-run-session -- python3 -I /ci/gk.py perf || status=$?
stop_display
exit "$status"
