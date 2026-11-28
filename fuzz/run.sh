#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# Runs the fuzz targets for a bounded time each, all at once, one process
# per target. Meant for a throwaway container of a CI image, for example:
#
#     podman run --rm --ulimit core=0 -v "$PWD:/src:z" -w /src \
#         -v gk-fuzz-target:/target -e CARGO_TARGET_DIR=/target \
#         localhost/glyphkeel-ci:fedora fuzz/run.sh 600
#
# Usage: fuzz/run.sh [SECONDS] [TARGET…]   (default: 600 seconds, every target)
#
# Each target starts from its seeds in fuzz/seeds/<target>/ (checked in,
# small) and keeps what it finds in fuzz/corpus/<target>/ (not checked in).
# Crashes land in fuzz/artifacts/<target>/; the logs in fuzz/logs/. The exit
# status is non-zero when any target failed.
set -eu

seconds=${1:-600}
[ $# -gt 0 ] && shift
cd "$(dirname "$0")"
targets=${*:-"query_parse markdown_render csv_import archive_import wiki_import mcp_message extract_pdf extract_office"}

rustup toolchain install nightly --profile minimal >/dev/null
# The newest cargo-fuzz: older ones pin crates that no longer build on nightly.
command -v cargo-fuzz >/dev/null || cargo +nightly install cargo-fuzz

mkdir -p logs
for target in $targets; do
  cargo +nightly fuzz build "$target"
done

pids=""
for target in $targets; do
  mkdir -p "corpus/$target" "seeds/$target" "artifacts/$target"
  (
    status=0
    cargo +nightly fuzz run "$target" "corpus/$target" "seeds/$target" -- \
      -max_total_time="$seconds" -timeout=30 -rss_limit_mb=4096 -max_len=65536 \
      -print_final_stats=1 >"logs/$target.log" 2>&1 || status=$?
    echo "$status" >"logs/$target.status"
  ) &
  pids="$pids $!"
done
wait $pids

failed=0
for target in $targets; do
  status=$(cat "logs/$target.status")
  runs=$(grep -o 'stat::number_of_executed_units: *[0-9]*' "logs/$target.log" | grep -o '[0-9]*$' || echo "?")
  cov=$(grep -o 'cov: [0-9]*' "logs/$target.log" | tail -1 || true)
  echo "$target: exit $status, $runs runs, $cov"
  [ "$status" = 0 ] || failed=1
done
exit $failed
