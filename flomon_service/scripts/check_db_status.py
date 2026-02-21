#!/usr/bin/env python3
"""Quick database status check script"""

import psycopg2
from datetime import datetime

# Connect to database
conn = psycopg2.connect(
    host="localhost",
    database="flopro_db",
    user="flopro_admin",
    password="flopro_dev_2026"
)

cur = conn.cursor()

print("=" * 80)
print("FLOMON SERVICE - DATABASE STATUS REPORT")
print("=" * 80)
print(f"Report generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")

# Total readings
cur.execute("SELECT COUNT(*) FROM usgs_raw.gauge_readings")
total_readings = cur.fetchone()[0]
print(f"📊 Total USGS readings in database: {total_readings:,}")

# Readings per station
print("\n📍 Readings by Station:")
print("-" * 80)
cur.execute("""
    SELECT site_code, 
           COUNT(*) as readings,
           MIN(measurement_time) as first_reading,
           MAX(measurement_time) as latest_reading,
           MAX(ingested_at) as last_ingestion
    FROM usgs_raw.gauge_readings
    GROUP BY site_code
    ORDER BY site_code
""")

for row in cur.fetchall():
    site_code, readings, first, latest, ingested = row
    print(f"  {site_code}: {readings:>7,} readings | Data: {first.date()} to {latest.date()} | Ingested: {ingested}")

# Parameters collected
print("\n📈 Parameters Collected:")
print("-" * 80)
cur.execute("""
    SELECT parameter_code, COUNT(*) as readings
    FROM usgs_raw.gauge_readings
    GROUP BY parameter_code
    ORDER BY parameter_code
""")

for row in cur.fetchall():
    param_code, readings = row
    param_name = "Stage (ft)" if param_code == "00065" else "Discharge (cfs)" if param_code == "00060" else "Unknown"
    print(f"  {param_code} ({param_name}): {readings:,} readings")

# Recent activity
print("\n🕒 Recent Activity (last 24 hours):")
print("-" * 80)
cur.execute("""
    SELECT COUNT(*) 
    FROM usgs_raw.gauge_readings
    WHERE ingested_at > NOW() - INTERVAL '1 day'
""")
recent_readings = cur.fetchone()[0]
print(f"  Readings ingested in last 24 hours: {recent_readings:,}")

# Data freshness
print("\n🌡️  Data Freshness:")
print("-" * 80)
cur.execute("""
    SELECT site_code,
           MAX(measurement_time) as latest_data,
           NOW() - MAX(measurement_time) as data_age
    FROM usgs_raw.gauge_readings
    GROUP BY site_code
    ORDER BY data_age
""")

for row in cur.fetchall():
    site_code, latest, age = row
    hours_old = age.total_seconds() / 3600
    status = "✓" if hours_old < 24 else "⚠" if hours_old < 168 else "❌"
    print(f"  {status} {site_code}: Latest data from {latest} ({hours_old:.1f} hours old)")

print("\n" + "=" * 80)

cur.close()
conn.close()
