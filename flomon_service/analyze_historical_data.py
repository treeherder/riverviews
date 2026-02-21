#!/usr/bin/env python3
"""
Analyze available historical data in the database
Shows what date ranges have good data coverage for development/testing
"""

import psycopg2
from datetime import datetime, timedelta
from collections import defaultdict

conn = psycopg2.connect(
    host="localhost",
    database="flopro_db",
    user="flopro_admin",
    password="flopro_dev_2026"
)

cur = conn.cursor()

print("=" * 80)
print("HISTORICAL DATA AVAILABILITY ANALYSIS")
print("=" * 80)
print(f"Analysis run: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")

# Get overall statistics
cur.execute("""
    SELECT 
        site_code,
        COUNT(*) as total_readings,
        MIN(measurement_time) as first_reading,
        MAX(measurement_time) as latest_reading,
        MAX(measurement_time) - MIN(measurement_time) as timespan
    FROM usgs_raw.gauge_readings
    GROUP BY site_code
    ORDER BY site_code
""")

print("📊 Data Coverage by Station:")
print("-" * 80)
print(f"{'Site Code':<12} {'Readings':>10} {'First Reading':<20} {'Latest Reading':<20} {'Span (days)':<12}")
print("-" * 80)

stations = []
for row in cur.fetchall():
    site_code, readings, first, latest, timespan = row
    stations.append(site_code)
    span_days = timespan.days if timespan else 0
    print(f"{site_code:<12} {readings:>10,} {str(first):<20} {str(latest):<20} {span_days:<12}")

# Find best periods for development (days with complete data)
print("\n\n📅 Best Days for Development (Complete 15-minute data):")
print("-" * 80)
print("Looking for days with 90+ readings (near-complete 15-min coverage)...\n")

cur.execute("""
    SELECT 
        DATE_TRUNC('day', measurement_time)::date as day,
        site_code,
        COUNT(*) as readings_per_day,
        COUNT(DISTINCT parameter_code) as parameters_available
    FROM usgs_raw.gauge_readings
    GROUP BY day, site_code
    HAVING COUNT(*) >= 90
    ORDER BY day DESC, site_code
    LIMIT 50
""")

results = cur.fetchall()
if results:
    # Group by day
    days_data = defaultdict(list)
    for day, site, readings, params in results:
        days_data[day].append((site, readings, params))
    
    # Show best days (those with most stations reporting)
    sorted_days = sorted(days_data.items(), key=lambda x: len(x[1]), reverse=True)
    
    print(f"{'Date':<15} {'Stations':>10} {'Details'}")
    print("-" * 80)
    
    for day, stations_list in sorted_days[:20]:
        station_count = len(stations_list)
        print(f"{str(day):<15} {station_count:>10} ", end="")
        
        # Show station codes
        station_codes = [s[0] for s in stations_list[:4]]
        print(f"{', '.join(station_codes)}", end="")
        if len(stations_list) > 4:
            print(f" +{len(stations_list)-4} more", end="")
        print()
else:
    print("  No days found with complete 15-minute coverage")

# Show data gaps
print("\n\n⚠️  Data Gaps (periods > 1 day with no data):")
print("-" * 80)

for site_code in stations[:3]:  # Check first 3 stations as examples
    cur.execute("""
        WITH gaps AS (
            SELECT 
                site_code,
                measurement_time,
                LAG(measurement_time) OVER (ORDER BY measurement_time) as prev_time,
                measurement_time - LAG(measurement_time) OVER (ORDER BY measurement_time) as gap
            FROM usgs_raw.gauge_readings
            WHERE site_code = %s
        )
        SELECT 
            site_code,
            prev_time,
            measurement_time,
            gap
        FROM gaps
        WHERE gap > INTERVAL '1 day'
        ORDER BY gap DESC
        LIMIT 5
    """, (site_code,))
    
    gaps = cur.fetchall()
    if gaps:
        print(f"\n{site_code}:")
        for site, start, end, gap in gaps:
            gap_days = gap.days
            print(f"  {str(start)[:19]} → {str(end)[:19]} ({gap_days} days)")
    else:
        print(f"\n{site_code}: No significant gaps found")

# Recommended development periods
print("\n\n✅ RECOMMENDED DEVELOPMENT PERIODS:")
print("-" * 80)
print("Best date ranges to use for testing/development:\n")

if sorted_days:
    best_day = sorted_days[0][0]
    best_station_count = len(sorted_days[0][1])
    
    # Find consecutive days around best day
    cur.execute("""
        SELECT DISTINCT DATE_TRUNC('day', measurement_time)::date as day
        FROM usgs_raw.gauge_readings
        WHERE measurement_time >= %s::date - INTERVAL '7 days'
          AND measurement_time <= %s::date + INTERVAL '7 days'
        ORDER BY day
    """, (best_day, best_day))
    
    consecutive_days = [row[0] for row in cur.fetchall()]
    
    print(f"📍 Primary Recommendation: {best_day}")
    print(f"   - {best_station_count} stations with complete data")
    print(f"   - Data available for ±7 day window")
    print(f"   - Date range: {consecutive_days[0]} to {consecutive_days[-1]}")
    print()
    print(f"   Example usage:")
    print(f"   ```rust")
    print(f"   let dev = DevMode::new({(datetime.now().date() - best_day).days});")
    print(f"   let readings = dev.fetch_simulated_current_readings(&mut client, &site_codes)?;")
    print(f"   ```")
    print()
    print(f"   Example SQL query:")
    print(f"   ```sql")
    print(f"   SELECT * FROM usgs_raw.gauge_readings")
    print(f"   WHERE measurement_time >= '{best_day} 00:00:00'")
    print(f"     AND measurement_time < '{best_day + timedelta(days=1)} 00:00:00'")
    print(f"   ORDER BY measurement_time, site_code;")
    print(f"   ```")

print("\n" + "=" * 80)

cur.close()
conn.close()
