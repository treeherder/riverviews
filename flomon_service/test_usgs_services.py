#!/usr/bin/env python3
"""
Test all three USGS data services to diagnose availability
Shows which services are working and what data is available
"""

import requests
import json
from datetime import datetime, timedelta

SITE_CODE = "05568500"  # Kingston Mines - primary reference station

print("=" * 80)
print("USGS DATA SERVICES DIAGNOSTIC")
print("=" * 80)
print(f"Station: {SITE_CODE} (Illinois River at Kingston Mines, IL)")
print(f"Test time: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")

# ---------------------------------------------------------------------------
# Test 1: IV Service (Instantaneous Values - Last 120 days)
# ---------------------------------------------------------------------------

print("\n" + "=" * 80)
print("TEST 1: IV SERVICE (Instantaneous Values)")
print("=" * 80)
print("Endpoint: https://waterservices.usgs.gov/nwis/iv/")
print("Time Range: Last 3 hours (should have 12-15 readings at 15-min intervals)")
print()

iv_url = (
    f"https://waterservices.usgs.gov/nwis/iv/"
    f"?sites={SITE_CODE}"
    f"&parameterCd=00060,00065"
    f"&period=PT3H"
    f"&format=json"
    f"&siteStatus=active"
)

print(f"URL: {iv_url}\n")

try:
    response = requests.get(iv_url, timeout=10)
    print(f"Status Code: {response.status_code}")
    
    if response.status_code == 200:
        data = response.json()
        time_series = data.get("value", {}).get("timeSeries", [])
        
        if time_series:
            print(f"✅ SUCCESS: {len(time_series)} parameter(s) available")
            
            for series in time_series:
                param = series["variable"]["variableCode"][0]["value"]
                param_name = series["variable"]["variableName"]
                values = series.get("values", [{}])[0].get("value", [])
                
                print(f"\n   Parameter {param} ({param_name}):")
                print(f"   - {len(values)} readings")
                
                if values:
                    latest = values[-1]
                    print(f"   - Latest: {latest['value']} {series['variable']['unit']['unitCode']}")
                    print(f"   - Time: {latest['dateTime']}")
                else:
                    print(f"   - ⚠️  No data returned (possible station outage)")
        else:
            print("❌ FAILED: No timeSeries entries in response")
            print("   Possible causes:")
            print("   - Station equipment failure")
            print("   - Station decommissioned")
            print("   - Temporary API outage")
    else:
        print(f"❌ FAILED: HTTP {response.status_code}")
        
except Exception as e:
    print(f"❌ ERROR: {e}")

# ---------------------------------------------------------------------------
# Test 2: DV Service (Daily Values - Full historical record)
# ---------------------------------------------------------------------------

print("\n\n" + "=" * 80)
print("TEST 2: DV SERVICE (Daily Values)")
print("=" * 80)
print("Endpoint: https://waterservices.usgs.gov/nwis/dv/")
print("Time Range: Last 30 days (should have 30 daily mean values)")
print()

end_date = datetime.now()
start_date = end_date - timedelta(days=30)

dv_url = (
    f"https://waterservices.usgs.gov/nwis/dv/"
    f"?sites={SITE_CODE}"
    f"&parameterCd=00060,00065"
    f"&startDT={start_date.strftime('%Y-%m-%d')}"
    f"&endDT={end_date.strftime('%Y-%m-%d')}"
    f"&format=json"
)

print(f"URL: {dv_url}\n")

try:
    response = requests.get(dv_url, timeout=10)
    print(f"Status Code: {response.status_code}")
    
    if response.status_code == 200:
        data = response.json()
        time_series = data.get("value", {}).get("timeSeries", [])
        
        if time_series:
            print(f"✅ SUCCESS: {len(time_series)} parameter(s) available")
            
            for series in time_series:
                param = series["variable"]["variableCode"][0]["value"]
                param_name = series["variable"]["variableName"]
                values = series.get("values", [{}])[0].get("value", [])
                
                print(f"\n   Parameter {param} ({param_name}):")
                print(f"   - {len(values)} daily readings")
                
                if values:
                    latest = values[-1]
                    print(f"   - Latest: {latest['value']} {series['variable']['unit']['unitCode']}")
                    print(f"   - Date: {latest['dateTime']}")
                    
                    first = values[0]
                    print(f"   - Oldest: {first['value']} {series['variable']['unit']['unitCode']}")
                    print(f"   - Date: {first['dateTime']}")
                else:
                    print(f"   - ⚠️  No data returned")
        else:
            print("❌ FAILED: No timeSeries entries in response")
    else:
        print(f"❌ FAILED: HTTP {response.status_code}")
        
except Exception as e:
    print(f"❌ ERROR: {e}")

# ---------------------------------------------------------------------------
# Test 3: Peak Service (Annual Peak Flows - Full historical record)
# ---------------------------------------------------------------------------

print("\n\n" + "=" * 80)
print("TEST 3: PEAK SERVICE (Annual Peak Streamflow)")
print("=" * 80)
print("Endpoint: https://nwis.waterdata.usgs.gov/{state}/nwis/peak")
print("Time Range: Full period of record (typically 50-100 years)")
print("Format: RDB (tab-delimited text, not JSON)")
print()

peak_url = f"https://nwis.waterdata.usgs.gov/il/nwis/peak?site_no={SITE_CODE}&agency_cd=USGS&format=rdb"

print(f"URL: {peak_url}\n")

try:
    response = requests.get(peak_url, timeout=10)
    print(f"Status Code: {response.status_code}")
    
    if response.status_code == 200:
        rdb_text = response.text
        lines = rdb_text.strip().split('\n')
        
        # Find data lines (skip comments and format lines)
        data_lines = [line for line in lines if not line.startswith('#') and '\t' in line]
        
        if len(data_lines) >= 2:  # Header + format + data
            header_line = data_lines[0]
            format_line = data_lines[1]  # RDB format specification
            data_records = data_lines[2:]  # Actual data
            
            print(f"✅ SUCCESS: {len(data_records)} annual peak records")
            print(f"   Format: RDB (USGS tab-delimited)")
            print(f"   Columns: {', '.join(header_line.split('\t')[:6])}")
            
            if data_records:
                # Parse a few recent records
                print(f"\n   Most Recent Peaks:")
                for line in data_records[-5:]:
                    fields = line.split('\t')
                    if len(fields) >= 7:
                        date = fields[2]
                        discharge = fields[4] if fields[4] else 'N/A'
                        stage = fields[6] if fields[6] else 'N/A'
                        print(f"   - {date}: {discharge} cfs, {stage} ft")
                
                # Count floods above thresholds
                flood_stage_ft = 16.0
                major_flood_ft = 24.0
                
                floods = 0
                major_floods = 0
                for line in data_records:
                    fields = line.split('\t')
                    if len(fields) >= 7 and fields[6]:
                        try:
                            stage = float(fields[6])
                            if stage >= flood_stage_ft:
                                floods += 1
                            if stage >= major_flood_ft:
                                major_floods += 1
                        except ValueError:
                            pass
                
                print(f"\n   Historical Flood Events:")
                print(f"   - Floods (≥{flood_stage_ft} ft): {floods}")
                print(f"   - Major Floods (≥{major_flood_ft} ft): {major_floods}")
                print(f"   - Period of Record: {len(data_records)} years")
        else:
            print("❌ FAILED: No peak flow records in response")
    else:
        print(f"❌ FAILED: HTTP {response.status_code}")
        
except Exception as e:
    print(f"❌ ERROR: {e}")

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

print("\n\n" + "=" * 80)
print("DIAGNOSTIC SUMMARY")
print("=" * 80)
print()
print("📊 Service Availability:")
print("   1. IV Service (Real-time, 15-min):    [Run test to see]")
print("   2. DV Service (Historical, daily):    [Run test to see]")
print("   3. Peak Service (Annual peaks):       [Run test to see]")
print()
print("💡 Tips:")
print("   - If IV fails but DV works: Station may be offline, use DV for now")
print("   - If DV works: You have access to full historical record")
print("   - Peak Service: Use for flood frequency analysis")
print()
print("📖 Documentation: docs/USGS_DATA_SERVICES.md")
print("=" * 80)
