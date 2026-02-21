#!/usr/bin/env python3
"""
Example: Query site data from flomon_service HTTP endpoint

Demonstrates how to use the endpoint from Python/FloML for analysis.
"""

import requests
import json
from datetime import datetime

ENDPOINT_URL = "http://localhost:8080"

def get_site_data(site_code):
    """Fetch all data for a monitoring station."""
    response = requests.get(f"{ENDPOINT_URL}/site/{site_code}")
    response.raise_for_status()
    return response.json()

def check_flood_status(site_code):
    """Check current flood status for a site."""
    data = get_site_data(site_code)
    
    print(f"\n{'='*60}")
    print(f"Site: {data['site_name']} ({data['site_code']})")
    print(f"{'='*60}")
    
    # Display current readings
    if data['stage']:
        stage = data['stage']
        print(f"\n📊 Stage: {stage['value']} {stage['unit']}")
        print(f"   Time: {stage['datetime']}")
        print(f"   Quality: {stage['qualifier']}")
    
    if data['discharge']:
        discharge = data['discharge']
        print(f"\n💧 Discharge: {discharge['value']:,.0f} {discharge['unit']}")
        print(f"   Time: {discharge['datetime']}")
        print(f"   Quality: {discharge['qualifier']}")
    
    # Check against thresholds
    if data['thresholds'] and data['stage']:
        stage_value = data['stage']['value']
        thresholds = data['thresholds']
        
        print(f"\n🌊 Flood Thresholds:")
        print(f"   Action:   {thresholds['action_stage_ft']} ft")
        print(f"   Flood:    {thresholds['flood_stage_ft']} ft")
        print(f"   Moderate: {thresholds['moderate_flood_stage_ft']} ft")
        print(f"   Major:    {thresholds['major_flood_stage_ft']} ft")
        
        print(f"\n⚠️  Flood Status:")
        if stage_value >= thresholds['major_flood_stage_ft']:
            print(f"   🔴 MAJOR FLOOD - Stage {stage_value} ft")
        elif stage_value >= thresholds['moderate_flood_stage_ft']:
            print(f"   🟠 MODERATE FLOOD - Stage {stage_value} ft")
        elif stage_value >= thresholds['flood_stage_ft']:
            print(f"   🟡 MINOR FLOOD - Stage {stage_value} ft")
        elif stage_value >= thresholds['action_stage_ft']:
            print(f"   🟢 ACTION STAGE - Stage {stage_value} ft")
        else:
            print(f"   ✅ NORMAL - Stage {stage_value} ft")
    
    # Display monitoring state
    if data['monitoring_state']:
        state = data['monitoring_state']
        print(f"\n📡 Monitoring State:")
        print(f"   Status: {state['status']}")
        print(f"   Stale: {state['is_stale']}")
        print(f"   Consecutive Failures: {state['consecutive_failures']}")
    
    # Display data freshness
    if data['staleness_minutes'] is not None:
        staleness = data['staleness_minutes']
        print(f"\n⏱️  Data Freshness:")
        if staleness < 60:
            print(f"   ✅ Fresh - {staleness} minutes old")
        elif staleness < 120:
            print(f"   ⚠️  Aging - {staleness} minutes old")
        else:
            print(f"   🔴 Stale - {staleness} minutes old ({staleness/60:.1f} hours)")
    
    print()

def list_all_stations():
    """Query all configured stations."""
    # This would need a /stations endpoint
    # For now, just demonstrate with known sites
    sites = [
        "05568500",  # Kingston Mines
        "05567500",  # Peoria Pool
        "05568000",  # Chillicothe
    ]
    
    print("\n" + "="*60)
    print("Station Summary")
    print("="*60)
    
    for site_code in sites:
        try:
            data = get_site_data(site_code)
            stage = data['stage']['value'] if data['stage'] else None
            discharge = data['discharge']['value'] if data['discharge'] else None
            
            print(f"\n{data['site_name']}")
            print(f"  Site Code: {data['site_code']}")
            if stage:
                print(f"  Stage: {stage} ft")
            if discharge:
                print(f"  Discharge: {discharge:,.0f} ft3/s")
            
            # Check staleness
            if data['staleness_minutes'] and data['staleness_minutes'] > 60:
                print(f"  ⚠️  Data is {data['staleness_minutes']} minutes old")
        except Exception as e:
            print(f"\n{site_code}: Error - {e}")

def main():
    """Main demo function."""
    print("FloPro Endpoint Demo")
    print("="*60)
    
    # Test health check
    try:
        health = requests.get(f"{ENDPOINT_URL}/health").json()
        print(f"\n✅ Service health: {health['status']}")
        print(f"   Version: {health['version']}")
    except Exception as e:
        print(f"\n❌ Cannot connect to endpoint: {e}")
        print(f"\nStart the daemon with:")
        print(f"  cargo run --release -- --endpoint 8080")
        return
    
    # Query specific station
    check_flood_status("05568500")  # Kingston Mines
    
    # List all stations
    # list_all_stations()

if __name__ == "__main__":
    main()
