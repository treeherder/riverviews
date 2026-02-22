#!/usr/bin/env python3
"""
Zone Sensor Visualization
=========================
Demonstrates the flood monitoring system's sensor architecture across zones.

Creates an interactive visualization showing:
- Geographic zones from property outward
- Sensor types (USGS, CWMS, ASOS) 
- Current readings and staleness
- Lead times for flood prediction
"""

import requests
import json
from datetime import datetime
from typing import Dict, List, Any
from collections import defaultdict

# Colors for terminal output
class Colors:
    HEADER = '\033[95m'
    BLUE = '\033[94m'
    CYAN = '\033[96m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    RED = '\033[91m'
    END = '\033[0m'
    BOLD = '\033[1m'
    UNDERLINE = '\033[4m'

def fetch_zone_data(zone_id: int, base_url: str = "http://localhost:8080") -> Dict[str, Any]:
    """Fetch zone data from the monitoring service."""
    response = requests.get(f"{base_url}/zone/{zone_id}")
    response.raise_for_status()
    return response.json()

def get_staleness_color(staleness_min: int) -> str:
    """Return color based on data staleness."""
    if staleness_min is None:
        return Colors.RED
    elif staleness_min < 30:
        return Colors.GREEN
    elif staleness_min < 120:
        return Colors.YELLOW
    else:
        return Colors.RED

def format_value(value: float, unit: str) -> str:
    """Format sensor value with appropriate precision."""
    if value is None:
        return "N/A"
    if unit == "ft":
        return f"{value:.2f} ft"
    elif unit == "cfs":
        return f"{value:,.0f} cfs"
    elif unit == "in":
        return f"{value:.2f} in"
    else:
        return f"{value} {unit}"

def visualize_zone(zone_id: int):
    """Create a detailed visualization of a single zone."""
    try:
        zone = fetch_zone_data(zone_id)
    except Exception as e:
        print(f"{Colors.RED}Error fetching zone {zone_id}: {e}{Colors.END}")
        return
    
    # Header
    print(f"\n{Colors.BOLD}{Colors.BLUE}{'='*80}{Colors.END}")
    print(f"{Colors.BOLD}{Colors.CYAN}ZONE {zone_id}: {zone.get('name', 'Unknown')}{Colors.END}")
    print(f"{Colors.BOLD}{Colors.BLUE}{'='*80}{Colors.END}")
    
    # Description
    if zone.get('description'):
        desc = zone['description']
        # Word wrap description
        words = desc.split()
        line = ""
        for word in words:
            if len(line) + len(word) + 1 <= 76:
                line += word + " "
            else:
                print(f"  {line}")
                line = word + " "
        if line:
            print(f"  {line}")
    
    # Sensor statistics
    sensors = zone.get('sensors', [])
    sensor_types = defaultdict(int)
    for sensor in sensors:
        source = sensor.get('source', 'Unknown')
        sensor_types[source] += 1
    
    print(f"\n{Colors.BOLD}Sensors: {len(sensors)} total{Colors.END}")
    for source, count in sorted(sensor_types.items()):
        icon = "🌊" if "USGS" in source else "🔒" if "USACE" in source else "☁️" if "ASOS" in source else "📡"
        print(f"  {icon}  {source}: {count} sensor(s)")
    
    # Sensor details grouped by role
    roles = defaultdict(list)
    for sensor in sensors:
        role = sensor.get('role', 'unknown')
        roles[role].append(sensor)
    
    print(f"\n{Colors.BOLD}Sensor Details:{Colors.END}")
    print(f"{Colors.BLUE}{'─'*80}{Colors.END}")
    
    for role in ['direct', 'boundary', 'precip', 'proxy']:
        if role not in roles:
            continue
            
        role_name = role.upper()
        print(f"\n{Colors.BOLD}{Colors.YELLOW}{role_name} MEASUREMENTS:{Colors.END}")
        
        for sensor in sorted(roles[role], key=lambda s: s.get('location', '')):
            sensor_id = sensor.get('sensor_id') or sensor.get('id', 'N/A')
            location = sensor.get('location', 'Unknown')
            source = sensor.get('source', 'Unknown')
            value = sensor.get('current_value')
            unit = sensor.get('current_unit', '')
            staleness = sensor.get('staleness_minutes')
            
            # Staleness indicator
            stale_color = get_staleness_color(staleness)
            stale_text = f"{staleness}m" if staleness is not None else "N/A"
            
            # Format current reading
            if value is not None:
                reading = format_value(value, unit)
                value_color = Colors.GREEN
            else:
                reading = "No data"
                value_color = Colors.RED
            
            print(f"  • {Colors.BOLD}{sensor_id}{Colors.END} - {location}")
            print(f"    {source} | {value_color}{reading}{Colors.END} | Age: {stale_color}{stale_text}{Colors.END}")
            
            # Add relevance if available
            if sensor.get('relevance') and len(sensor['relevance']) < 100:
                print(f"    💡 {sensor['relevance'][:97]}...")

def create_system_overview():
    """Create a system-wide overview of all zones."""
    print(f"\n{Colors.BOLD}{Colors.HEADER}{'='*80}")
    print(f"🌊 ILLINOIS RIVER FLOOD MONITORING SYSTEM - SENSOR OVERVIEW")
    print(f"{'='*80}{Colors.END}\n")
    
    print(f"{Colors.CYAN}Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}{Colors.END}")
    
    # Fetch all zones
    zones_data = []
    for zone_id in range(7):
        try:
            data = fetch_zone_data(zone_id)
            zones_data.append((zone_id, data))
        except:
            continue
    
    # Summary table
    print(f"\n{Colors.BOLD}Zone Summary:{Colors.END}")
    print(f"{'Zone':<6} {'Name':<45} {'Sensors':<10} {'Data Age'}")
    print(f"{Colors.BLUE}{'─'*80}{Colors.END}")
    
    for zone_id, zone in zones_data:
        name = zone.get('name', 'Unknown')[:43]
        sensor_count = len(zone.get('sensors', []))
        
        # Average staleness
        staleness_values = [s.get('staleness_minutes') for s in zone.get('sensors', []) 
                           if s.get('staleness_minutes') is not None]
        if staleness_values:
            avg_staleness = sum(staleness_values) / len(staleness_values)
            stale_text = f"{avg_staleness:.0f}m avg"
            stale_color = get_staleness_color(avg_staleness)
        else:
            stale_text = "N/A"
            stale_color = Colors.RED
        
        print(f"{zone_id:<6} {name:<45} {sensor_count:<10} {stale_color}{stale_text}{Colors.END}")
    
    # Detailed zone views
    for zone_id, _ in zones_data:
        visualize_zone(zone_id)

def create_sensor_map():
    """Create a geographic sensor map visualization."""
    print(f"\n{Colors.BOLD}{Colors.HEADER}{'='*80}")
    print(f"📍 SENSOR GEOGRAPHIC DISTRIBUTION")
    print(f"{'='*80}{Colors.END}\n")
    
    all_sensors = []
    for zone_id in range(7):
        try:
            zone = fetch_zone_data(zone_id)
            for sensor in zone.get('sensors', []):
                coords = sensor.get('coordinates', {})
                if coords.get('lat') and coords.get('lon'):
                    sensor['zone_id'] = zone_id
                    sensor['zone_name'] = zone.get('name', '')
                    all_sensors.append(sensor)
        except:
            continue
    
    # Sort by latitude (north to south)
    all_sensors.sort(key=lambda s: s.get('coordinates', {}).get('lat', 0), reverse=True)
    
    print(f"{'Sensor':<12} {'Location':<35} {'Coordinates':<25} {'Zone'}")
    print(f"{Colors.BLUE}{'─'*100}{Colors.END}")
    
    for sensor in all_sensors:
        sensor_id = (sensor.get('sensor_id') or sensor.get('id', 'N/A'))[:10]
        location = sensor.get('location', 'Unknown')[:33]
        coords = sensor.get('coordinates', {})
        lat = coords.get('lat', 0)
        lon = coords.get('lon', 0)
        zone_id = sensor.get('zone_id', '?')
        
        coord_str = f"{lat:.3f}°N, {lon:.3f}°W"
        
        # Color by source
        source = sensor.get('source', '')
        if 'USGS' in source:
            color = Colors.CYAN
        elif 'USACE' in source:
            color = Colors.BLUE
        elif 'ASOS' in source:
            color = Colors.GREEN
        else:
            color = Colors.END
        
        print(f"{color}{sensor_id:<12}{Colors.END} {location:<35} {coord_str:<25} Zone {zone_id}")

def main():
    """Main demonstration entry point."""
    import sys
    
    if len(sys.argv) > 1:
        if sys.argv[1] == 'overview':
            create_system_overview()
        elif sys.argv[1] == 'map':
            create_sensor_map()
        elif sys.argv[1].isdigit():
            zone_id = int(sys.argv[1])
            visualize_zone(zone_id)
        else:
            print(f"Usage: {sys.argv[0]} [overview|map|<zone_id>]")
            sys.exit(1)
    else:
        # Default: show everything
        create_system_overview()
        print("\n")
        create_sensor_map()
        
        print(f"\n{Colors.BOLD}{Colors.GREEN}Visualization complete!{Colors.END}")
        print(f"\n{Colors.YELLOW}Try:{Colors.END}")
        print(f"  python3 {sys.argv[0]} overview   # System overview only")
        print(f"  python3 {sys.argv[0]} map        # Geographic map only")
        print(f"  python3 {sys.argv[0]} 2          # Detailed zone 2 view")

if __name__ == "__main__":
    main()
