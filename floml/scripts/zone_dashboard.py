#!/usr/bin/env python3
"""
Riverviews Flood Monitoring Dashboard
======================================
Ncurses terminal dashboard for the flomon_service zone-based API.

Modes
-----
  GRID   — 7-zone overview with alert levels, key readings, freshness
  DETAIL — full scrollable sensor table for one zone (with threshold bars)

Key bindings
------------
  q / Q       quit (or return to grid from detail)
  ESC         return to grid
  r / R       force refresh
  0-6         open detail view for that zone
  ← →         prev / next zone in detail
  ↑ ↓         scroll sensor list in detail

Usage
-----
  python zone_dashboard.py [--api http://HOST:PORT]
"""

import argparse
import curses
import sys
import time
import traceback
from datetime import datetime
from typing import Dict, List, Optional

import requests

# ── Configuration ─────────────────────────────────────────────────────────────

NUM_ZONES       = 7
REFRESH_SEC     = 30
STALE_FRESH_MIN = 30    # green below this
STALE_WARN_MIN  = 120   # yellow below this, red above


# ── Data cache ────────────────────────────────────────────────────────────────

class DataCache:
    def __init__(self, api_base: str):
        self.base = api_base
        self.status: Optional[Dict]       = None
        self.zones:  Dict[int, Dict]      = {}
        self.last_fetch:  float           = 0
        self.error:       Optional[str]   = None

    def fetch_all(self):
        """Blocking fetch of /status + all /zone/{n}. Called in main loop."""
        try:
            r = requests.get(f"{self.base}/status", timeout=5)
            r.raise_for_status()
            self.status = r.json()
            self.error = None
        except Exception as e:
            self.status = None
            self.error = str(e)

        for i in range(NUM_ZONES):
            try:
                r = requests.get(f"{self.base}/zone/{i}", timeout=5)
                r.raise_for_status()
                self.zones[i] = r.json()
            except Exception as e:
                self.zones[i] = {"zone_id": i, "error": str(e)}

        self.last_fetch = time.time()

    def next_refresh_in(self) -> int:
        return max(0, int(REFRESH_SEC - (time.time() - self.last_fetch)))


# ── Formatting helpers ─────────────────────────────────────────────────────────

def fmt_val(value: Optional[float], unit: Optional[str]) -> str:
    if value is None:
        return "N/A"
    u = (unit or "").lower()
    if "cfs" in u:
        return f"{value/1000:.1f}k cfs" if value >= 1000 else f"{value:.0f} cfs"
    if "in" in u:
        return f"{value:.2f} in"
    return f"{value:.2f} ft"


def fmt_stale(minutes: Optional[int]) -> str:
    if minutes is None:
        return "  N/A"
    if minutes < 60:
        return f"{minutes:3d}m"
    return f"{minutes//60}h{minutes%60:02d}m"


def lead_str(lo: Optional[int], hi: Optional[int]) -> str:
    if lo is None and hi is None:
        return "  --  "
    if hi is not None and hi >= 120:
        return f"{lo}h-{hi//24}d"
    if lo is None:
        return f"<{hi}h"
    if hi is None:
        return f">{lo}h"
    return f"{lo}-{hi}h"


def stage_bar(value: Optional[float], flood: Optional[float], width: int = 8) -> str:
    """Filled-block bar showing stage as fraction of flood stage."""
    if value is None or not flood:
        return " " * width
    pct = min(value / flood, 1.0)
    filled = round(pct * width)
    return "█" * filled + "░" * (width - filled)


# ── Color helpers ─────────────────────────────────────────────────────────────

def init_colors() -> Dict[str, int]:
    pairs: Dict[str, int] = {}
    try:
        curses.start_color()
        curses.use_default_colors()
        curses.init_pair(1, curses.COLOR_GREEN,   -1)
        curses.init_pair(2, curses.COLOR_YELLOW,  -1)
        curses.init_pair(3, curses.COLOR_RED,     -1)
        curses.init_pair(4, curses.COLOR_CYAN,    -1)
        curses.init_pair(5, curses.COLOR_BLUE,    -1)
        curses.init_pair(6, curses.COLOR_MAGENTA, -1)
        curses.init_pair(7, curses.COLOR_WHITE,   -1)
        curses.init_pair(8, curses.COLOR_BLACK,   curses.COLOR_CYAN)
        pairs = {
            "green":  curses.color_pair(1),
            "yellow": curses.color_pair(2),
            "red":    curses.color_pair(3),
            "cyan":   curses.color_pair(4),
            "blue":   curses.color_pair(5),
            "magenta":curses.color_pair(6),
            "white":  curses.color_pair(7),
            "inv":    curses.color_pair(8),
            "dim":    curses.color_pair(7) | curses.A_DIM,
        }
    except Exception:
        for k in ("green","yellow","red","cyan","blue","magenta","white","inv","dim"):
            pairs[k] = 0
    return pairs


def alert_attr(level: str, P: Dict[str, int]) -> int:
    lvl = (level or "NORMAL").upper()
    if lvl in ("CRITICAL", "WARNING", "FLOOD_WARNING"):
        return P["red"] | curses.A_BOLD
    if lvl in ("WATCH", "ELEVATED", "FLOOD_WATCH"):
        return P["yellow"] | curses.A_BOLD
    return P["green"]


def stale_attr(minutes: Optional[int], P: Dict[str, int]) -> int:
    if minutes is None:
        return P["red"]
    if minutes < STALE_FRESH_MIN:
        return P["green"]
    if minutes < STALE_WARN_MIN:
        return P["yellow"]
    return P["red"]


# ── Safe draw primitives ──────────────────────────────────────────────────────

def sa(win, y: int, x: int, text: str, attr: int = 0):
    """addstr that clips to window and swallows curses.error."""
    try:
        h, w = win.getmaxyx()
        if y < 0 or y >= h or x < 0 or x >= w:
            return
        clipped = text[:max(0, w - x - 1)]
        if clipped:
            win.addstr(y, x, clipped, attr)
    except curses.error:
        pass


def hline(win, y: int, x: int, length: int, attr: int = 0):
    sa(win, y, x, "─" * length, attr)


def draw_box(win, y: int, x: int, h: int, w: int, attr: int = 0, title: str = ""):
    if h < 2 or w < 4:
        return
    sa(win, y,     x, "┌" + "─" * (w-2) + "┐", attr)
    sa(win, y+h-1, x, "└" + "─" * (w-2) + "┘", attr)
    for i in range(1, h-1):
        sa(win, y+i, x,     "│", attr)
        sa(win, y+i, x+w-1, "│", attr)
    if title:
        sa(win, y, x+2, f" {title} ", attr | curses.A_BOLD)


# ── Header (rows 0-1) ─────────────────────────────────────────────────────────

def draw_header(win, cache: DataCache, P: Dict[str, int], refreshing: bool):
    _, w = win.getmaxyx()
    title = "  ILLINOIS RIVER FLOOD MONITOR  "
    ts    = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    spin  = " ↻ " if refreshing else "   "

    sa(win, 0, 0, " " * w, P["inv"])
    sa(win, 0, 1, title[:w-2], P["inv"] | curses.A_BOLD)
    sa(win, 0, max(2, w - len(ts) - 5), ts + spin, P["inv"])

    st = cache.status
    if st:
        overall  = st.get("overall_status", "UNKNOWN")
        bw       = st.get("backwater_risk", {}).get("risk_level", "?")
        compound = st.get("compound_event_risk", "?")
        pulse    = st.get("upstream_flood_pulse", {})
        pulse_s  = (f"Pulse ~{pulse.get('estimated_arrival_hours', '?')}h"
                    if pulse.get("pulse_detected") else "Pulse: none")

        sa(win, 1, 1,  "Basin: ",        P["dim"])
        sa(win, 1, 8,  f"{overall:<12}", alert_attr(overall, P))
        sa(win, 1, 21, "│",              P["dim"])
        sa(win, 1, 23, "Backwater: ",    P["dim"])
        sa(win, 1, 34, f"{bw:<10}",      alert_attr(bw, P))
        sa(win, 1, 45, "│",              P["dim"])
        sa(win, 1, 47, f"{pulse_s:<20}", P["dim"])
        sa(win, 1, 68, "│",              P["dim"])
        sa(win, 1, 70, f"Compound: {compound}", P["dim"])
    elif cache.error:
        sa(win, 1, 1, f"API error: {cache.error[:w-4]}", P["red"])
    else:
        sa(win, 1, 1, "Connecting…", P["yellow"])


# ── Zone card (grid mode) ─────────────────────────────────────────────────────

def draw_zone_card(win, y: int, x: int, h: int, w: int,
                   zone: Dict, P: Dict[str, int]):
    zid  = zone.get("zone_id", "?")
    name = zone.get("zone_name") or zone.get("name") or "Unknown"
    star = " ★" if zid == 2 else ""

    if zone.get("error"):
        draw_box(win, y, x, h, w, P["red"], f"[{zid}]")
        sa(win, y+2, x+2, "NO DATA", P["red"] | curses.A_BOLD)
        return

    status  = zone.get("zone_status", {})
    level   = status.get("alert_level", "NORMAL")
    border  = alert_attr(level, P)
    sensors = zone.get("sensors", [])
    meta    = zone.get("metadata", {})
    active  = status.get("active_sensors", len(sensors))
    stale_n = status.get("stale_sensors", 0)

    title = f"[{zid}] {name[:w-8]}{star}"
    draw_box(win, y, x, h, w, border, title)

    # Line 1: lead time + status + sensor count
    lead = lead_str(meta.get("lead_time_hours_min"), meta.get("lead_time_hours_max"))
    s_badge = f" ⚠{stale_n}" if stale_n else ""
    sa(win, y+1, x+2, f"{lead:<9}{level:<9}{active}sens{s_badge}", border)

    line = 2
    shown = {"pool": False, "stage": False, "flow": False, "precip": False}

    for s in sensors:
        if line >= h - 2:
            break
        stype = s.get("sensor_type", "")
        val   = s.get("current_value")
        unit  = s.get("current_unit", "ft")
        flood = s.get("flood_stage_ft")

        if stype == "pool_elevation" and not shown["pool"]:
            sa(win, y+line, x+2, f"Pool  {fmt_val(val, unit)}", P["cyan"])
            shown["pool"] = True;  line += 1

        elif stype in ("stage", "stage_discharge") and not shown["stage"]:
            bar = stage_bar(val, flood, 6) if flood else "      "
            col = P["red"] if val and flood and val >= flood else P["white"]
            sa(win, y+line, x+2, f"Stage {fmt_val(val, unit)} {bar}", col)
            shown["stage"] = True;  line += 1

        elif "discharge" in stype and not shown["flow"]:
            sa(win, y+line, x+2, f"Flow  {fmt_val(val, 'cfs')}", P["white"])
            shown["flow"] = True;  line += 1

        elif stype in ("precipitation", "precip_grid") and not shown["precip"]:
            col = P["blue"] if (val or 0) > 0.0 else P["dim"]
            sa(win, y+line, x+2, f"Precip {fmt_val(val, 'in')}", col)
            shown["precip"] = True;  line += 1

    # Threshold alerts
    above_flood  = status.get("sensors_above_flood",  [])
    above_action = status.get("sensors_above_action", [])
    if above_flood and line < h - 2:
        sa(win, y+line, x+2, f"⚠ FLOOD ({len(above_flood)})", P["red"] | curses.A_BOLD)
        line += 1
    elif above_action and line < h - 2:
        sa(win, y+line, x+2, f"▲ action ({len(above_action)})", P["yellow"])
        line += 1

    # Freshness footer in last content row
    staleness = [s.get("staleness_minutes") for s in sensors
                 if s.get("staleness_minutes") is not None]
    if staleness:
        best = min(staleness)
        sc = stale_attr(best, P)
        sa(win, y+h-2, x+2, f"↻ {fmt_stale(best)}", sc)


# ── Zone detail view ──────────────────────────────────────────────────────────

def draw_zone_detail(win, zone: Dict, P: Dict[str, int], scroll: int):
    h, w = win.getmaxyx()

    if zone.get("error"):
        sa(win, 3, 2, f"Zone error: {zone['error']}", P["red"])
        return

    zid     = zone.get("zone_id", "?")
    name    = zone.get("zone_name") or zone.get("name") or "?"
    status  = zone.get("zone_status", {})
    level   = status.get("alert_level", "NORMAL")
    active  = status.get("active_sensors", 0)
    stale_n = status.get("stale_sensors", 0)
    sensors = zone.get("sensors", [])
    meta    = zone.get("metadata", {})
    desc    = zone.get("description", "")

    lead = lead_str(meta.get("lead_time_hours_min"), meta.get("lead_time_hours_max"))

    # Sub-header row
    sa(win, 2, 2, f"Zone {zid}: {name}", P["white"] | curses.A_BOLD)
    info = f"  Lead: {lead}  │  {active}✓ {stale_n}⚠"
    sa(win, 2, 2 + len(f"Zone {zid}: {name}"), info, P["dim"])
    sa(win, 2, w-16, f"Status: {level:<8}", alert_attr(level, P))

    hline(win, 3, 1, w-2, P["dim"])

    # Column header
    COL = {"loc": 2, "type": 30, "val": 47, "bar": 58, "stale": 70, "src": 77}
    header = (f"{'SENSOR':<27}  {'TYPE':<16}  {'VALUE':>9}  "
              f"{'FLOOD▶':>10}  {'STALE':>5}  SRC")
    sa(win, 4, 2, header[:w-3], P["dim"] | curses.A_BOLD)
    hline(win, 5, 1, w-2, P["dim"])

    above_flood  = set(status.get("sensors_above_flood",  []))
    above_action = set(status.get("sensors_above_action", []))

    row = 6
    for s in sensors[scroll:]:
        if row >= h - 4:
            remaining = len(sensors) - scroll - (row - 6)
            if remaining > 0:
                sa(win, row, 2, f"  ↓ {remaining} more  (↓ to scroll)", P["dim"])
            break

        sid   = s.get("sensor_id", "")
        loc   = (s.get("location") or sid)[:27]
        stype = s.get("sensor_type", "")[:16]
        val   = s.get("current_value")
        unit  = s.get("current_unit", "ft")
        flood = s.get("flood_stage_ft")
        act   = s.get("action_stage_ft")
        stale = s.get("staleness_minutes")
        src   = (s.get("source") or "")[:4]

        val_s   = f"{fmt_val(val, unit):>9}"
        bar_s   = stage_bar(val, flood, 8) if flood else " " * 8
        flood_s = f"{flood:.1f}ft" if flood else (f"act:{act:.1f}" if act else "   --  ")
        stale_s = f"{fmt_stale(stale):>5}"

        if sid in above_flood:
            row_attr = P["red"] | curses.A_BOLD
        elif sid in above_action:
            row_attr = P["yellow"] | curses.A_BOLD
        else:
            row_attr = stale_attr(stale, P)

        line = (f"  {loc:<27}  {stype:<16}  {val_s}  "
                f"{bar_s} {flood_s:<8}  {stale_s}  {src}")
        sa(win, row, 2, line[:w-3], row_attr)
        row += 1

    hline(win, row, 1, w-2, P["dim"])

    # Description (wrapped)
    if desc and row + 2 < h - 2:
        dr = row + 1
        words = desc.split()
        buf = ""
        for word in words:
            if len(buf) + len(word) + 1 > w - 6:
                sa(win, dr, 3, buf, P["dim"]); dr += 1; buf = word
                if dr >= h - 2:
                    break
            else:
                buf = f"{buf} {word}".strip()
        if buf and dr < h - 2:
            sa(win, dr, 3, buf, P["dim"])


# ── Grid view ─────────────────────────────────────────────────────────────────

def draw_grid(win, cache: DataCache, P: Dict[str, int]):
    h, w = win.getmaxyx()
    grid_top = 3       # rows 0–1 header, row 2 separator

    if w < 60 or h < 15:
        sa(win, 3, 2, f"Terminal too small ({w}×{h}), need 60×15", P["red"])
        return

    col_count = 3
    col_w     = (w - 4) // col_count
    avail_h   = h - grid_top - 2      # leave footer row + 1
    row_h     = max(7, (avail_h - 5) // 2)   # two rows of normal cards
    z6_h      = max(4, min(6, avail_h - row_h * 2 - 1))

    for zid in range(6):
        r, c  = divmod(zid, col_count)
        cy    = grid_top + r * row_h
        cx    = 2 + c * col_w
        zone  = cache.zones.get(zid, {"zone_id": zid, "error": "not loaded"})
        draw_zone_card(win, cy, cx, row_h - 1, col_w - 1, zone, P)

    z6   = cache.zones.get(6, {"zone_id": 6, "error": "not loaded"})
    z6_y = grid_top + 2 * row_h
    draw_zone_card(win, z6_y, 2, z6_h, w - 4, z6, P)


# ── Footer ────────────────────────────────────────────────────────────────────

def draw_footer(win, cache: DataCache, P: Dict[str, int], mode: str):
    h, w = win.getmaxyx()
    nxt = cache.next_refresh_in()
    if mode == "GRID":
        txt = f" [q]quit  [r]refresh  [0-6] zone detail  Next: {nxt}s "
    else:
        txt = f" [ESC/q] grid  [r]refresh  [←→] prev/next  [↑↓] scroll  Next: {nxt}s "
    sa(win, h-1, 0, " " * (w-1), P["dim"])
    sa(win, h-1, 0, txt[:w-1],   P["dim"])


# ── Main loop ─────────────────────────────────────────────────────────────────

def run(stdscr, api_base: str):
    P = init_colors()
    curses.curs_set(0)
    stdscr.timeout(500)

    cache        = DataCache(api_base)
    mode         = "GRID"
    detail_zone  = 0
    detail_scroll= 0
    refreshing   = False

    def do_refresh():
        nonlocal refreshing
        refreshing = True
        _redraw()
        cache.fetch_all()
        refreshing = False

    def _redraw():
        stdscr.erase()
        draw_header(stdscr, cache, P, refreshing)
        _, w = stdscr.getmaxyx()
        hline(stdscr, 2, 0, w, P["dim"])
        if mode == "GRID":
            draw_grid(stdscr, cache, P)
        else:
            z = cache.zones.get(detail_zone, {"zone_id": detail_zone, "error": "not loaded"})
            draw_zone_detail(stdscr, z, P, detail_scroll)
        draw_footer(stdscr, cache, P, mode)
        stdscr.refresh()

    do_refresh()

    while True:
        _redraw()

        if cache.next_refresh_in() == 0:
            do_refresh()
            continue

        key = stdscr.getch()
        if key == -1:
            continue

        if key in (ord('q'), ord('Q')):
            if mode == "DETAIL":
                mode = "GRID"; detail_scroll = 0
            else:
                break

        elif key == 27:              # ESC
            mode = "GRID"; detail_scroll = 0

        elif key in (ord('r'), ord('R')):
            do_refresh()

        elif ord('0') <= key <= ord('6'):
            z = key - ord('0')
            if mode == "DETAIL" and z == detail_zone:
                mode = "GRID"; detail_scroll = 0
            else:
                detail_zone = z; detail_scroll = 0; mode = "DETAIL"

        elif mode == "DETAIL":
            sensors = cache.zones.get(detail_zone, {}).get("sensors", [])
            h, _ = stdscr.getmaxyx()
            max_scroll = max(0, len(sensors) - (h - 10))
            if key == curses.KEY_DOWN:
                detail_scroll = min(detail_scroll + 1, max_scroll)
            elif key == curses.KEY_UP:
                detail_scroll = max(0, detail_scroll - 1)
            elif key == curses.KEY_RIGHT:
                detail_zone = (detail_zone + 1) % NUM_ZONES
                detail_scroll = 0
            elif key == curses.KEY_LEFT:
                detail_zone = (detail_zone - 1) % NUM_ZONES
                detail_scroll = 0


# ── Entry point ───────────────────────────────────────────────────────────────

def _parse_args() -> str:
    p = argparse.ArgumentParser(description="Riverviews flood monitoring dashboard")
    p.add_argument("--api", default="http://localhost:8080",
                   help="flomon_service base URL (default: http://localhost:8080)")
    return p.parse_args().api


def main():
    api = _parse_args()
    try:
        curses.wrapper(run, api)
    except KeyboardInterrupt:
        print("\nDashboard closed.")
    except requests.exceptions.ConnectionError:
        print(f"❌  Cannot connect to flomon_service at {api}")
        print("    Ensure the daemon is running, or use --api to specify the URL.")
        sys.exit(1)
    except Exception as e:
        with open("/tmp/dashboard_error.log", "w") as f:
            traceback.print_exc(file=f)
        print(f"❌  {e}")
        print("    Full trace: cat /tmp/dashboard_error.log")
        sys.exit(1)


if __name__ == "__main__":
    main()

