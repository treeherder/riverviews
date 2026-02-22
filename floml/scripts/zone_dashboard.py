#!/usr/bin/env python3
"""
Zone Dashboard - Terminal Grid View
====================================
Displays all 7 zones in a grid layout with real-time sensor data.
"""

import curses
import requests
import time
import json
import hashlib
from datetime import datetime
from typing import Dict, List, Any

API_BASE = "http://localhost:8080"

def get_data_hash(zones: List[Dict[str, Any]]) -> str:
    """Compute a hash of the zone data to detect changes."""
    try:
        # Extract just the values we display
        display_data = []
        for zone in zones:
            zone_summary = {
                'zone_id': zone.get('zone_id'),
                'name': zone.get('name'),
                'error': zone.get('error', False),
            }
            if not zone.get('error'):
                sensors = []
                for s in zone.get('sensors', []):
                    sensors.append({
                        'id': s.get('sensor_id') or s.get('id'),
                        'value': s.get('current_value'),
                        'staleness': s.get('staleness_minutes'),
                        'source': s.get('source'),
                        'type': s.get('sensor_type'),
                    })
                zone_summary['sensors'] = sensors
            display_data.append(zone_summary)
        
        # Hash the JSON representation
        data_str = json.dumps(display_data, sort_keys=True)
        return hashlib.md5(data_str.encode()).hexdigest()
    except:
        return str(time.time())  # Fallback to always redraw on error

def fetch_all_zones() -> List[Dict[str, Any]]:
    """Fetch data for all zones."""
    zones = []
    for zone_id in range(7):
        try:
            response = requests.get(f"{API_BASE}/zone/{zone_id}", timeout=2)
            if response.status_code == 200:
                zones.append(response.json())
            else:
                zones.append({"zone_id": zone_id, "error": True})
        except Exception:
            zones.append({"zone_id": zone_id, "error": True})
    return zones

def get_sensor_summary(zone: Dict[str, Any]) -> Dict[str, Any]:
    """Extract key sensor info from zone data."""
    if not zone or zone.get('error'):
        return {
            'total': 0,
            'usgs': 0,
            'cwms': 0,
            'asos': 0,
            'freshest_min': 9999,
            'avg_stage': None,
            'total_precip': None,
            'avg_discharge': None,
        }
    
    sensors = zone.get('sensors', [])
    if not sensors:
        return {
            'total': 0,
            'usgs': 0,
            'cwms': 0,
            'asos': 0,
            'freshest_min': 9999,
            'avg_stage': None,
            'total_precip': None,
            'avg_discharge': None,
        }
    
    # Count by source
    usgs_count = 0
    cwms_count = 0
    asos_count = 0
    
    for s in sensors:
        source = s.get('source', '') or ''
        if 'USGS' in source:
            usgs_count += 1
        elif 'CWMS' in source or 'MVR' in source:
            cwms_count += 1
        elif 'ASOS' in source:
            asos_count += 1
    
    # Find freshest data
    staleness_values = []
    for s in sensors:
        staleness = s.get('staleness_minutes')
        if staleness is not None and isinstance(staleness, (int, float)):
            staleness_values.append(staleness)
    freshest = min(staleness_values) if staleness_values else 9999
    
    # Find stage sensors
    stage_values = []
    for s in sensors:
        sensor_type = s.get('sensor_type', '')
        # Include both 'stage' and 'stage_discharge' sensors
        if sensor_type in ('stage', 'stage_discharge', 'pool_elevation'):
            val = s.get('current_value')
            if val is not None and isinstance(val, (int, float)):
                stage_values.append(val)
    avg_stage = sum(stage_values) / len(stage_values) if stage_values else None
    
    # Find precip sensors
    precip_values = []
    for s in sensors:
        source = s.get('source', '') or ''
        if 'ASOS' in source:
            val = s.get('current_value')
            if val is not None and isinstance(val, (int, float)):
                precip_values.append(val)
    total_precip = sum(precip_values) if precip_values else None
    
    # Find discharge sensors
    discharge_values = []
    for s in sensors:
        sensor_type = s.get('sensor_type', '')
        if 'discharge' in sensor_type:
            val = s.get('current_value')
            if val is not None and isinstance(val, (int, float)):
                discharge_values.append(val)
    avg_discharge = sum(discharge_values) / len(discharge_values) if discharge_values else None
    
    return {
        'total': len(sensors),
        'usgs': usgs_count,
        'cwms': cwms_count,
        'asos': asos_count,
        'freshest_min': freshest,
        'avg_stage': avg_stage,
        'total_precip': total_precip,
        'avg_discharge': avg_discharge,
    }

def get_staleness_color(minutes: int) -> int:
    """Get color pair for staleness."""
    try:
        if minutes < 30:
            return curses.color_pair(1) or 0  # Green
        elif minutes < 120:
            return curses.color_pair(2) or 0  # Yellow
        else:
            return curses.color_pair(3) or 0  # Red
    except:
        return 0

def draw_zone_box(win, y: int, x: int, width: int, height: int, zone: Dict[str, Any], color_pair: int):
    """Draw a single zone box."""
    zone_id = zone.get('zone_id', '?')
    zone_name = zone.get('name', 'Unknown')
    
    # Truncate name if too long
    max_name_len = width - 4
    if len(zone_name) > max_name_len:
        zone_name = zone_name[:max_name_len-3] + "..."
    
    if zone.get('error'):
        # Error box
        try:
            win.addstr(y, x, "+" + "-" * (width - 2) + "+", color_pair)
            for i in range(1, height - 1):
                win.addstr(y + i, x, "|" + " " * (width - 2) + "|", color_pair)
            win.addstr(y + height - 1, x, "+" + "-" * (width - 2) + "+", color_pair)
            
            title = f" Zone {zone_id} "
            win.addstr(y, x + 2, title, curses.A_BOLD | color_pair)
            win.addstr(y + 2, x + 2, "ERROR", (curses.color_pair(3) or 0) | curses.A_BOLD)
        except curses.error:
            pass
        return
    
    # Normal box - use ASCII characters
    try:
        win.addstr(y, x, "+" + "-" * (width - 2) + "+", color_pair)
        for i in range(1, height - 1):
            win.addstr(y + i, x, "|" + " " * (width - 2) + "|", color_pair)
        win.addstr(y + height - 1, x, "+" + "-" * (width - 2) + "+", color_pair)
    except curses.error:
        pass
    
    # Title
    title = f" Zone {zone_id}: {zone_name} "
    if len(title) > width - 2:
        title = f" Zone {zone_id} "
    try:
        win.addstr(y, x + 2, title, curses.A_BOLD | color_pair)
    except curses.error:
        pass
    
    # Get summary
    summary = get_sensor_summary(zone)
    
    # Display data
    line = 1
    
    # Sensor counts
    sensor_line = f"Sensors: {summary['total']}"
    try:
        win.addstr(y + line, x + 2, sensor_line, color_pair)
        line += 1
    except curses.error:
        return
    
    if summary['usgs'] > 0:
        try:
            win.addstr(y + line, x + 2, f"  USGS: {summary['usgs']}", curses.color_pair(4) or 0)
            line += 1
        except curses.error:
            return
    if summary['cwms'] > 0:
        try:
            win.addstr(y + line, x + 2, f"  CWMS: {summary['cwms']}", curses.color_pair(5) or 0)
            line += 1
        except curses.error:
            return
    if summary['asos'] > 0:
        try:
            win.addstr(y + line, x + 2, f"  ASOS: {summary['asos']}", curses.color_pair(6) or 0)
            line += 1
        except curses.error:
            return
    
    # Stage
    if summary['avg_stage'] is not None and line < height - 2:
        stage_str = f"Stage: {summary['avg_stage']:.2f} ft"
        try:
            win.addstr(y + line, x + 2, stage_str, color_pair)
            line += 1
        except curses.error:
            return
    
    # Discharge
    if summary.get('avg_discharge') is not None and line < height - 2:
        discharge = summary['avg_discharge']
        if discharge >= 1000:
            discharge_str = f"Flow: {discharge/1000:.1f}k cfs"
        else:
            discharge_str = f"Flow: {discharge:.0f} cfs"
        try:
            win.addstr(y + line, x + 2, discharge_str, color_pair)
            line += 1
        except curses.error:
            return
    
    # Precip
    if summary['total_precip'] is not None and line < height - 2:
        precip_str = f"Precip: {summary['total_precip']:.2f} in"
        try:
            precip_color = (curses.color_pair(6) or 0) if summary['total_precip'] > 0 else color_pair
            precip_attr = (precip_color | curses.A_BOLD) if summary['total_precip'] > 0 else precip_color
            win.addstr(y + line, x + 2, precip_str, precip_attr)
            line += 1
        except curses.error:
            return
    
    # Freshness indicator
    if line < height - 2:
        freshness = summary['freshest_min']
        if freshness < 9999:
            fresh_color = get_staleness_color(freshness)
            fresh_str = f"Fresh: {freshness}m"
            try:
                win.addstr(y + line, x + 2, fresh_str, fresh_color)
            except curses.error:
                pass
            try:
                win.addstr(y + line, x + 2, fresh_str, curses.color_pair(fresh_color))
            except curses.error:
                pass

def draw_dashboard(stdscr, zones: List[Dict[str, Any]], last_fetch: float = 0):
    """Draw the full dashboard."""
    stdscr.clear()
    height, width = stdscr.getmaxyx()
    
    # Check minimum size
    if height < 20 or width < 80:
        try:
            stdscr.addstr(0, 0, "Terminal too small!", curses.A_BOLD | (curses.color_pair(3) or 0))
            stdscr.addstr(1, 0, f"Current: {width}x{height}, Need: 80x20 minimum")
            stdscr.addstr(2, 0, "Please resize your terminal window")
        except curses.error:
            pass
        stdscr.refresh()
        return
    
    # Header
    header = "ILLINOIS RIVER FLOOD MONITORING - ZONE DASHBOARD"
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    
    try:
        stdscr.addstr(0, max(0, (width - len(header)) // 2), header, curses.A_BOLD | (curses.color_pair(7) or 0))
        stdscr.addstr(1, max(0, width - len(timestamp) - 2), timestamp, curses.color_pair(7) or 0)
    except curses.error:
        pass
    
    # Calculate grid layout
    # 7 zones in a grid: 3x3 (with 2 empty spots)
    cols = 3
    rows = 3
    
    box_width = (width - 4) // cols
    box_height = (height - 4) // rows
    
    # Draw zones
    zone_positions = [
        (0, 0),  # Zone 0
        (0, 1),  # Zone 1
        (0, 2),  # Zone 2
        (1, 0),  # Zone 3
        (1, 1),  # Zone 4
        (1, 2),  # Zone 5
        (2, 0),  # Zone 6
    ]
    
    for idx, zone in enumerate(zones):
        if idx >= len(zone_positions):
            break
        
        row, col = zone_positions[idx]
        y = 3 + row * box_height
        x = 2 + col * box_width
        
        # Color based on zone importance
        color = curses.color_pair(7) or 0
        # Uncomment to highlight Zone 2 (property):
        # if zone.get('zone_id') == 2:
        #     color = (curses.color_pair(1) or 0) | curses.A_BOLD
        
        try:
            draw_zone_box(stdscr, y, x, box_width - 2, box_height - 1, zone, color)
        except curses.error:
            pass  # Ignore drawing errors from terminal size
    
    # Footer
    next_update = 30 - int(time.time() - last_fetch) if last_fetch > 0 else 0
    footer = f"Press 'q' to quit | 'r' to refresh | Next update in {max(0, next_update)}s"
    try:
        stdscr.addstr(height - 1, 2, footer, curses.color_pair(7) or 0)
    except curses.error:
        pass
    
    stdscr.refresh()

def main(stdscr):
    """Main dashboard loop."""
    # Initialize colors
    try:
        curses.start_color()
        if curses.has_colors():
            curses.use_default_colors()
            curses.init_pair(1, curses.COLOR_GREEN, -1)
            curses.init_pair(2, curses.COLOR_YELLOW, -1)
            curses.init_pair(3, curses.COLOR_RED, -1)
            curses.init_pair(4, curses.COLOR_CYAN, -1)
            curses.init_pair(5, curses.COLOR_BLUE, -1)
            curses.init_pair(6, curses.COLOR_MAGENTA, -1)
            curses.init_pair(7, curses.COLOR_WHITE, -1)
    except:
        # Colors not supported, continue without them
        pass
    
    # Non-blocking input
    stdscr.nodelay(True)
    curses.curs_set(0)  # Hide cursor
    
    last_fetch = 0
    zones = []
    last_hash = None  # Track data hash to detect changes
    
    while True:
        # Fetch data every 30 seconds
        current_time = time.time()
        if current_time - last_fetch > 30 or not zones:
            try:
                # Show brief loading message without clearing screen
                height, width = stdscr.getmaxyx()
                loading_msg = " Fetching data... "
                try:
                    stdscr.addstr(0, max(0, width - len(loading_msg) - 2), loading_msg, curses.A_BOLD)
                    stdscr.refresh()
                except:
                    pass
                
                zones = fetch_all_zones()
                last_fetch = current_time
            except Exception as e:
                # Display error
                stdscr.clear()
                stdscr.addstr(0, 0, f"Error fetching data: {e}", curses.color_pair(3) or 0)
                stdscr.addstr(2, 0, "Press 'q' to quit, 'r' to retry")
                stdscr.refresh()
                time.sleep(1)
                last_hash = None  # Force redraw after error
        
        # Check if data has changed
        current_hash = get_data_hash(zones) if zones else None
        if current_hash != last_hash:
            # Data changed, redraw
            try:
                draw_dashboard(stdscr, zones, last_fetch)
                last_hash = current_hash
            except Exception as e:
                stdscr.clear()
                stdscr.addstr(0, 0, f"Error drawing: {e}", curses.color_pair(3) or 0)
                stdscr.refresh()
        
        # Handle input
        try:
            key = stdscr.getch()
            if key == ord('q') or key == ord('Q'):
                break
            elif key == ord('r') or key == ord('R'):
                last_fetch = 0  # Force refresh
        except:
            pass
        
        time.sleep(0.5)  # Check for changes every 500ms

if __name__ == "__main__":
    import traceback
    try:
        curses.wrapper(main)
    except KeyboardInterrupt:
        print("\nDashboard closed.")
    except requests.exceptions.ConnectionError:
        print("❌ Error: Cannot connect to monitoring daemon")
        print("   Make sure the daemon is running on port 8080")
    except Exception as e:
        # Write full error to log file
        with open('/tmp/dashboard_error.log', 'w') as f:
            f.write(f"Error type: {type(e).__name__}\n")
            f.write(f"Error message: {e}\n\n")
            f.write("Full traceback:\n")
            traceback.print_exc(file=f)
        
        print(f"\n❌ Error: {e}")
        print(f"   Full error written to: /tmp/dashboard_error.log")
        print(f"   Run: cat /tmp/dashboard_error.log")
