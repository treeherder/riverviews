#!/usr/bin/env python3
"""
Sensor Correlation Demonstration
=================================
Demonstrates analysis capabilities by showing correlations between
ASOS precipitation and USGS gauge stage changes.
"""

import requests
from datetime import datetime, timedelta
import json

def fetch_recent_asos_data():
    """Fetch recent ASOS precipitation data."""
    print("📊 SENSOR CORRELATION ANALYSIS")
    print("=" * 70)
    print()
    
    # Fetch zone 2 (property zone) which has both ASOS and stage data
    response = requests.get("http://localhost:8080/zone/2")
    zone = response.json()
    
    print(f"🎯 Analysis Zone: {zone.get('name', 'Zone 2')}")
    print()
    
    # Find ASOS and stage sensors
    asos_sensors = [s for s in zone.get('sensors', []) if 'ASOS' in s.get('source', '')]
    stage_sensors = [s for s in zone.get('sensors', []) if s.get('sensor_type') == 'stage']
    
    print("📡 PRECIPITATION SENSORS:")
    for sensor in asos_sensors:
        sensor_id = sensor.get('sensor_id', 'N/A')
        location = sensor.get('location', 'Unknown')
        value = sensor.get('current_value')
        staleness = sensor.get('staleness_minutes')
        
        print(f"  • {sensor_id} ({location})")
        if value is not None:
            print(f"    Current: {value:.2f} in")
        if staleness:
            print(f"    Last updated: {staleness} minutes ago")
    
    print()
    print("🌊 STAGE SENSORS:")
    for sensor in stage_sensors:
        sensor_id = sensor.get('sensor_id') or sensor.get('id', 'N/A')
        location = sensor.get('location', 'Unknown')
        value = sensor.get('current_value')
        staleness = sensor.get('staleness_minutes')
        
        print(f"  • {sensor_id} ({location})")
        if value is not None:
            print(f"    Current: {value:.2f} ft")
        if staleness:
            print(f"    Last updated: {staleness} minutes ago")
    
    print()
    print("💡 CORRELATION INSIGHTS:")
    print("-" * 70)
    print()
    
    # Check for recent precipitation
    precip_detected = False
    for sensor in asos_sensors:
        if sensor.get('current_value', 0) > 0:
            precip_detected = True
            break
    
    if precip_detected:
        print("  ⚠️  PRECIPITATION DETECTED!")
        print()
        print("  Expected stage response:")
        print("    • Kingston Mines (05568500): 6-12 hour lag")
        print("    • Peoria pool (05567500): 12-24 hour lag")
        print()
        print("  💧 Local runoff contribution:")
        print("    • East bank (Woodford County): Immediate surface runoff")
        print("    • Mackinaw River tributary: 6-12 hour lag")
    else:
        print("  ✓ No recent precipitation detected")
        print()
        print("  Current stage readings represent:")
        print("    • Upstream flow contributions")
        print("    • Peoria L&D pool management")
        print("    • Mississippi backwater influence")
    
    print()
    print("📈 TREND ANALYSIS:")
    print("-" * 70)
    print()
    
    # Compare sensors
    if len(stage_sensors) >= 2:
        peoria_pool = next((s for s in stage_sensors if 'Peoria' in s.get('location', '')), None)
        kingston = next((s for s in stage_sensors if 'Kingston' in s.get('location', '')), None)
        
        if peoria_pool and kingston:
            pool_val = peoria_pool.get('current_value')
            km_val = kingston.get('current_value')
            
            if pool_val is not None and km_val is not None:
                diff = km_val - pool_val
                print(f"  Kingston Mines vs Peoria Pool differential: {diff:.2f} ft")
                print()
                
                if abs(diff) < 2:
                    print("  ✓ Normal gradient - free flow conditions")
                elif diff > 2:
                    print("  ⚠️  Positive gradient - upstream flow dominant")
                    print("     Possible flood risk from upstream contributions")
                else:
                    print("  ⚠️  Negative gradient - backwater dominant")
                    print("     Mississippi backwater may be controlling pool")
    
    print()
    print("=" * 70)
    print("🔬 Analysis complete")
    print()
    print("For historical correlation analysis, use:")
    print("  python3 analyze_events.py")

def main():
    try:
        fetch_recent_asos_data()
    except requests.exceptions.ConnectionError:
        print("❌ Error: Cannot connect to monitoring daemon")
        print("   Make sure the daemon is running on port 8080")
        return 1
    except Exception as e:
        print(f"❌ Error: {e}")
        return 1
    
    return 0

if __name__ == "__main__":
    import sys
    sys.exit(main())
