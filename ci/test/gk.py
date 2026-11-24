# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
"""Drives the installed app, the command line and the MCP server for the
package checks (`cargo xtask ci --suite offline,perf`). Standard library
only, so it runs on every distribution's own Python.

  gk.py mcp-probe KEY   a short MCP session: read, search and write
  gk.py perf            the performance measurements with data set L;
                        writes /out/perf.json and /out/perf.md
"""

import json
import os
import signal
import statistics
import subprocess
import sys
import time
import urllib.request

OUT = "/out"
APP = "/usr/bin/glyphkeel-app"
WINDOW_NAME = "Glyphkeel"


# ---------------------------------------------------------------- helpers


def run(args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, text=True, **kwargs)


def ms(seconds):
    return round(seconds * 1000, 1)


def p95(values):
    values = sorted(values)
    return values[min(len(values) - 1, round(0.95 * (len(values) - 1)))]


def descendants(root):
    """The PIDs of `root` and every process below it."""
    children = {}
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            with open(f"/proc/{entry}/stat", encoding="utf-8") as stat:
                # The name may hold spaces and brackets: split after the last ")".
                fields = stat.read().rsplit(")", 1)[1].split()
        except OSError:
            continue
        children.setdefault(int(fields[1]), []).append(int(entry))
    found, todo = [], [root]
    while todo:
        pid = todo.pop()
        found.append(pid)
        todo.extend(children.get(pid, []))
    return found


def pss_mb(root):
    """Proportional set size of a process tree, in MB, and its parts."""
    total, parts = 0, []
    for pid in descendants(root):
        try:
            with open(f"/proc/{pid}/comm", encoding="utf-8") as comm:
                name = comm.read().strip()
            with open(f"/proc/{pid}/smaps_rollup", encoding="utf-8") as rollup:
                kb = next(int(line.split()[1]) for line in rollup if line.startswith("Pss:"))
        except (OSError, StopIteration):
            continue
        total += kb
        parts.append(f"{name} {kb / 1024:.0f}")
    return round(total / 1024, 1), parts


def window_visible():
    return (
        subprocess.run(
            ["xdotool", "search", "--onlyvisible", "--name", WINDOW_NAME],
            capture_output=True,
            check=False,
        ).returncode
        == 0
    )


def find_pid(name):
    for entry in os.listdir("/proc"):
        if entry.isdigit():
            try:
                with open(f"/proc/{entry}/comm", encoding="utf-8") as comm:
                    if comm.read().strip() == name[:15]:
                        return int(entry)
            except OSError:
                pass
    return None


# -------------------------------------------------------------------- MCP


class Mcp:
    """A client of `glyphkeel mcp` over standard input and output."""

    def __init__(self):
        self.started = time.monotonic()
        self.proc = subprocess.Popen(
            ["glyphkeel", "mcp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.next_id = 0

    def request(self, method, params):
        self.next_id += 1
        message = {"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}
        self.proc.stdin.write(json.dumps(message) + "\n")
        self.proc.stdin.flush()
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("the MCP server closed its output")
            answer = json.loads(line)
            if answer.get("id") == self.next_id:
                if "error" in answer:
                    raise RuntimeError(f"{method}: {answer['error']}")
                return answer["result"]

    def notify(self, method):
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method}) + "\n")
        self.proc.stdin.flush()

    def initialize(self):
        result = self.request(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "package-checks", "version": "1"},
            },
        )
        self.notify("notifications/initialized")
        return result

    def call(self, tool, arguments):
        result = self.request("tools/call", {"name": tool, "arguments": arguments})
        if result.get("isError"):
            raise RuntimeError(f"{tool}: {result}")
        return result

    def close(self):
        self.proc.stdin.close()
        self.proc.wait(timeout=10)


def mcp_probe(key):
    mcp = Mcp()
    mcp.initialize()
    mcp.call("get_workspace", {})
    mcp.call("get_task", {"keys": [key]})
    mcp.call("search", {"text": "report"})
    project = key.split("-")[0]
    mcp.call("create_task", {"project": project, "title": "Made over MCP while offline"})
    mcp.close()
    print("mcp-probe: passed")


# -------------------------------------------------------------- WebDriver

ELEMENT = "element-6066-11e4-a52e-4f735466cecf"


class WebDriver:
    """WebKitGTK's WebDriver, which starts the app with automation on."""

    def __init__(self, port, args):
        env = dict(os.environ, TAURI_WEBVIEW_AUTOMATION="true")
        self.log = open(f"{OUT}/webdriver.log", "w", encoding="utf-8")  # noqa: SIM115
        self.driver = subprocess.Popen(
            ["WebKitWebDriver", f"--port={port}"], env=env, stdout=self.log, stderr=self.log
        )
        self.base = f"http://127.0.0.1:{port}"
        for _ in range(100):
            try:
                self.http("GET", "/status")
                break
            except OSError:
                time.sleep(0.1)
        caps = {"webkitgtk:browserOptions": {"binary": APP, "args": args}}
        self.started = time.monotonic()
        session = self.http("POST", "/session", {"capabilities": {"alwaysMatch": caps}})
        self.session = f"/session/{session['sessionId']}"

    def http(self, method, path, body=None):
        data = None if body is None else json.dumps(body).encode()
        request = urllib.request.Request(
            self.base + path,
            data=data,
            method=method,
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(request, timeout=120) as response:
            return json.loads(response.read() or b"{}").get("value")

    def script(self, source, *args):
        return self.http("POST", f"{self.session}/execute/sync", {"script": source, "args": list(args)})

    def script_async(self, source, *args):
        return self.http("POST", f"{self.session}/execute/async", {"script": source, "args": list(args)})

    def find(self, css):
        return self.http("POST", f"{self.session}/element", {"using": "css selector", "value": css})

    def click(self, element):
        self.http("POST", f"{self.session}/element/{element[ELEMENT]}/click", {})

    def actions(self, actions):
        self.http("POST", f"{self.session}/actions", {"actions": actions})

    def wait_for(self, source, timeout=60):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if self.script(source):
                return time.monotonic()
            time.sleep(0.01)
        raise RuntimeError(f"timed out waiting for: {source}")

    def app_pid(self):
        return find_pid("glyphkeel-app")

    def quit(self):
        try:
            self.http("DELETE", self.session)
        finally:
            self.driver.send_signal(signal.SIGTERM)
            self.driver.wait(timeout=10)
            self.log.close()


# Frame times while `body` runs in the page: every requestAnimationFrame
# callback's time stamp, from start to stop.
FRAMES_START = """
window.__gkFrames = [];
window.__gkStop = false;
const tick = (t) => { window.__gkFrames.push(t); if (!window.__gkStop) requestAnimationFrame(tick); };
requestAnimationFrame(tick);
"""
FRAMES_STOP = "window.__gkStop = true; return window.__gkFrames;"

# Scrolls the element by `step` pixels a frame for `frames` frames and
# returns the frame time stamps.
SCROLL = """
const [selector, step, frames, done] = arguments;
const candidates = [...document.querySelectorAll(selector)];
const scrollable = (el) => el.scrollHeight > el.clientHeight + 10 &&
  ["auto", "scroll"].includes(getComputedStyle(el).overflowY);
let target = null;
for (const root of candidates) {
  for (const el of [root, ...root.querySelectorAll("*")]) {
    if (scrollable(el) && (!target || el.scrollHeight > target.scrollHeight)) target = el;
  }
}
if (!target) { done({ error: "nothing to scroll in " + selector }); return; }
const stamps = [];
let n = 0;
const tick = (t) => {
  stamps.push(t);
  target.scrollTop += step;
  if (++n < frames) requestAnimationFrame(tick);
  else done({ stamps, height: target.scrollHeight, rows: target.querySelectorAll("[role=row], [data-card]").length });
};
requestAnimationFrame(tick);
"""


def frame_stats(stamps):
    deltas = [b - a for a, b in zip(stamps, stamps[1:])]
    if not deltas:
        return {"fps": 0, "p95_ms": None, "long_frames": None, "frames": 0}
    seconds = (stamps[-1] - stamps[0]) / 1000
    return {
        "fps": round(len(deltas) / seconds, 1),
        "p95_ms": round(p95(deltas), 1),
        "long_frames": sum(1 for d in deltas if d > 33.4),
        "frames": len(deltas),
    }


# ------------------------------------------------------------------- perf


def cli_latency(key, project):
    show, listing = [], []
    for _ in range(10):
        t = time.monotonic()
        run(["glyphkeel", "task", "show", key])
        show.append(time.monotonic() - t)
    for _ in range(10):
        t = time.monotonic()
        run(["glyphkeel", "task", "list", "--project", project])
        listing.append(time.monotonic() - t)
    return {
        "task_show_first_ms": ms(show[0]),
        "task_show_median_ms": ms(statistics.median(show)),
        "task_list_median_ms": ms(statistics.median(listing)),
    }


def mcp_latency(key, project):
    mcp = Mcp()
    mcp.initialize()
    first = time.monotonic() - mcp.started
    get_task, search_tasks = [], []
    for _ in range(30):
        t = time.monotonic()
        mcp.call("get_task", {"keys": [key]})
        get_task.append(time.monotonic() - t)
    for _ in range(30):
        t = time.monotonic()
        mcp.call("search_tasks", {"project": project, "limit": 50})
        search_tasks.append(time.monotonic() - t)
    mcp.close()
    return {
        "mcp_first_answer_ms": ms(first),
        "mcp_get_task_p95_ms": ms(p95(get_task)),
        "mcp_search_tasks_p95_ms": ms(p95(search_tasks)),
    }


def start_and_idle(link, runs=5, settle=15):
    """Launch to visible window, without automation; then idle memory."""
    times, idle = [], None
    for index in range(runs):
        started = time.monotonic()
        with open(f"{OUT}/app-start-{index}.log", "w", encoding="utf-8") as log:
            app = subprocess.Popen([APP, link], stdout=log, stderr=log)
        while not window_visible():
            if app.poll() is not None:
                raise RuntimeError("the app exited during start")
            if time.monotonic() - started > 60:
                raise RuntimeError("no window after 60 s")
            time.sleep(0.005)
        times.append(time.monotonic() - started)
        if index == runs - 1:
            time.sleep(settle)
            idle = pss_mb(app.pid)
        app.terminate()
        app.wait(timeout=20)
        with open(f"{OUT}/app-start-{index}.log", encoding="utf-8") as log:
            if "did not signal its first screen" in log.read():
                raise RuntimeError("the page never drew its first screen")
        time.sleep(1)
    return {
        "start_window_first_ms": ms(times[0]),
        "start_window_median_ms": ms(statistics.median(times[1:] or times)),
        "idle_pss_mb": idle[0],
        "idle_pss_parts": idle[1],
    }


def drive(link):
    """The board and the list with automation: paint, scroll, drag, memory."""
    results = {}
    driver = WebDriver(4444, [link])
    try:
        first_card = driver.wait_for("return document.querySelector('[data-card]') !== null")
        results["start_first_card_ms"] = ms(first_card - driver.started)
        time.sleep(2)

        # Board: scroll the longest column.
        out = driver.script_async(SCROLL, "section[data-lane]", 60, 180)
        if "error" in out:
            raise RuntimeError(out["error"])
        results["board_scroll"] = frame_stats(out["stamps"])

        # Board: drag the first card of the first column onto the second column.
        lanes = driver.script("return document.querySelectorAll('section[data-lane]').length")
        card = driver.find("section[data-lane] [data-card]")
        key = driver.script("return arguments[0].getAttribute('data-card')", card)
        target = driver.find("section[data-lane]:nth-of-type(2) [data-card]")
        driver.script(FRAMES_START)
        driver.actions(
            [
                {
                    "type": "pointer",
                    "id": "mouse",
                    "parameters": {"pointerType": "mouse"},
                    "actions": [
                        {"type": "pointerMove", "origin": card, "x": 0, "y": 0, "duration": 0},
                        {"type": "pointerDown", "button": 0},
                        {"type": "pointerMove", "origin": "pointer", "x": 12, "y": 12, "duration": 100},
                        {"type": "pointerMove", "origin": target, "x": 0, "y": 0, "duration": 1500},
                        {"type": "pause", "duration": 200},
                        {"type": "pointerUp", "button": 0},
                    ],
                }
            ]
        )
        stamps = driver.script(FRAMES_STOP)
        moved = driver.script(
            "const c = document.querySelector(`[data-card='${arguments[0]}']`);"
            "const lanes = [...document.querySelectorAll('section[data-lane]')];"
            "return c ? lanes.indexOf(c.closest('section[data-lane]')) : -1;",
            key,
        )
        results["board_drag"] = dict(frame_stats(stamps), card=key, lanes=lanes, landed_in_lane=moved)

        # List: open it and scroll through the 20,000 rows.
        # The header's views: Board, then List.
        link_to_list = driver.find(".seg[role=group] button:nth-child(2)")
        clicked = time.monotonic()
        driver.click(link_to_list)
        shown = driver.wait_for("return document.querySelectorAll('[role=grid] [role=row]').length > 5")
        results["list_open_ms"] = ms(shown - clicked)
        time.sleep(1)
        out = driver.script_async(SCROLL, "[role=grid]", 400, 300)
        if "error" in out:
            raise RuntimeError(out["error"])
        results["list_scroll"] = dict(frame_stats(out["stamps"]), height_px=out["height"])

        # Back to the board: how long until its cards are drawn.
        link_to_board = driver.find(".seg[role=group] button:nth-child(1)")
        clicked = time.monotonic()
        driver.click(link_to_board)
        shown = driver.wait_for("return document.querySelector('section[data-lane] [data-card]') !== null")
        results["board_open_ms"] = ms(shown - clicked)
        time.sleep(5)
        pid = driver.app_pid()
        if pid:
            results["after_use_pss_mb"], results["after_use_pss_parts"] = pss_mb(pid)
    finally:
        driver.quit()
    return results


BUDGETS = [
    # key, label, budget, unit, lower is better
    ("start_window_median_ms", "Start to the window with the board (median of 4 warm starts)", 2000, "ms"),
    ("start_window_first_ms", "Start to the window with the board (first start)", 2000, "ms"),
    ("start_first_card_ms", "Start to the first card in the page (with automation)", 2000, "ms"),
    ("idle_pss_mb", "Idle memory of all app processes (PSS)", 300, "MB"),
    ("after_use_pss_mb", "Memory after board and list use (PSS)", 450, "MB"),
    ("board_open_ms", "Board of 1,000 open cards opens", 500, "ms"),
    ("board_scroll.p95_ms", "Board scroll, p95 frame time", 20.8, "ms"),
    ("board_drag.p95_ms", "Board drag, p95 frame time", 20.8, "ms"),
    ("list_open_ms", "List of 20,000 tasks opens", 500, "ms"),
    ("list_scroll.p95_ms", "List scroll over 20,000 rows, p95 frame time", 20.8, "ms"),
    ("mcp_first_answer_ms", "MCP first answer after start", 200, "ms"),
    ("mcp_get_task_p95_ms", "MCP get_task, p95", 50, "ms"),
    ("mcp_search_tasks_p95_ms", "MCP search_tasks, p95", 150, "ms"),
    ("task_show_first_ms", "CLI task show, first run", 100, "ms"),
    ("task_list_median_ms", "CLI task list over 20,000 tasks (first page)", 300, "ms"),
]


def lookup(results, dotted):
    value = results
    for part in dotted.split("."):
        if not isinstance(value, dict) or part not in value:
            return None
        value = value[part]
    return value


def table(results):
    lines = ["| Measure | Result | Budget | Verdict |", "|---|---|---|---|"]
    failed = []
    for key, label, budget, unit in BUDGETS:
        value = lookup(results, key)
        if value is None:
            verdict, shown = "not measured", "–"
        else:
            verdict = "within" if value <= budget else "OVER"
            shown = f"{value} {unit}"
            if value > budget:
                failed.append(label)
        lines.append(f"| {label} | {shown} | < {budget} {unit} | {verdict} |")
    for name in ("board_scroll", "board_drag", "list_scroll"):
        stats = results.get(name)
        if stats:
            lines.append(
                f"| {name.replace('_', ' ')}: frames per second | {stats['fps']} "
                f"({stats['long_frames']} of {stats['frames']} frames over 33 ms) | 60 | – |"
            )
    return "\n".join(lines) + "\n", failed


def perf():
    project = "BULK"
    key = f"{project}-1"
    link = f"glyphkeel://open/{key}"
    results = {"data_set": "L (seed 1)"}
    t = time.monotonic()
    run(["glyphkeel", "import", "/data/L"])
    results["import_L_s"] = round(time.monotonic() - t, 1)
    results.update(cli_latency(key, project))
    results.update(mcp_latency(key, project))
    results.update(start_and_idle(link))
    try:
        results.update(drive(link))
    except Exception as err:  # noqa: BLE001 - reported in the results
        results["drive_error"] = str(err)
    with open(f"{OUT}/perf.json", "w", encoding="utf-8") as out:
        json.dump(results, out, indent=2)
    text, failed = table(results)
    if "drive_error" in results:
        text += f"\nThe automated board and list run failed: {results['drive_error']}\n"
    for part in ("idle_pss_parts", "after_use_pss_parts"):
        if part in results:
            text += f"\n{part.replace('_', ' ')}: {', '.join(results[part])}\n"
    with open(f"{OUT}/perf.md", "w", encoding="utf-8") as out:
        out.write(text)
    print(text)
    if os.environ.get("PERF_ENFORCE") == "1" and (failed or "drive_error" in results):
        print(f"perf: FAILED: over budget: {', '.join(failed) or 'none'}")
        return 1
    print("perf: done")
    return 0


def main():
    if sys.argv[1:2] == ["mcp-probe"]:
        mcp_probe(sys.argv[2])
        return 0
    if sys.argv[1:2] == ["perf"]:
        return perf()
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
