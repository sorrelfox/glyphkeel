# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
"""Reads an `strace -f` log and fails on any IPv4 or IPv6 address in a
connect, bind or send call. Local sockets and netlink are allowed.

--require NAME: a program of that name must have been started under the
trace, so the check proves it covered that process.
"""

import argparse
import os
import re
import sys

CALL = re.compile(r"^(\d+)\s+(?:<\.\.\. )?(connect|bind|sendto|sendmsg|sendmmsg)\b")
INET = re.compile(r"sa_family=AF_INET6?\b")
EXEC = re.compile(r'^(\d+)\s+execve\("([^"]+)"')


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace")
    parser.add_argument("--require", action="append", default=[])
    args = parser.parse_args()

    programs: dict[str, str] = {}
    started: set[str] = set()
    hits: list[str] = []
    calls = 0
    with open(args.trace, encoding="utf-8", errors="replace") as log:
        for line in log:
            if match := EXEC.match(line):
                pid, path = match.groups()
                if "= -1" not in line or "<unfinished" in line:
                    programs[pid] = os.path.basename(path)
                    started.add(os.path.basename(path))
                continue
            if match := CALL.match(line):
                calls += 1
                if INET.search(line):
                    pid = match.group(1)
                    hits.append(f"{programs.get(pid, '?')} (pid {pid}): {line.strip()}")

    print(f"offline: {calls} socket calls traced; programs started: {', '.join(sorted(started))}")
    missing = [name for name in args.require if name not in started]
    if missing:
        print(f"offline: FAILED: never started under the trace: {', '.join(missing)}")
        return 1
    if hits:
        print(f"offline: FAILED: {len(hits)} calls with a network address:")
        for hit in hits[:50]:
            print(f"  {hit}")
        return 1
    print("offline: passed: no IPv4 or IPv6 address in any call")
    return 0


if __name__ == "__main__":
    sys.exit(main())
