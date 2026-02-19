# Data Storage Strategy: Separating Measurements from Monitoring Metadata

## Design Principle: Only Store Valid Readings

**Core Rule:** The `gauge_readings` table contains ONLY actual valid measurements from USGS gauges. It NEVER contains "no data" records, null values, or placeholders.

**Absence of data is tracked separately** in the `monitoring_state` table as part of the staleness monitoring subsystem.

---

## Data Flow: Readings Only

### 1. API Request
```rust
// Request data from USGS API
let url = build_iv_url(&sites, &params, "PT1H");
let response = reqwest::blocking::get(&url)?.text()?;
```

### 2. Parse Response (Filter Out Invalid Data)
```rust
// parse_iv_response() and parse_dv_response() implement:
for series in timeSeries {
    // SKIP empty value arrays
    if values_wrapper.value.is_empty() {
        continue; // Don't add to readings, try next series
    }
    
    // SKIP sentinel values (-999999)
    if (value - no_data_value).abs() < 0.1 {
        continue; // Don't add to readings, try next series
    }
    
    // ONLY add valid measurements
    readings.push(GaugeReading {
        site_code,
        parameter_code,
        value,        // Actual measured value
        datetime,     // Actual measurement timestamp
        ...
    });
}

// If NO valid readings found, return error
if readings.is_empty() {
    return Err(NwisError::NoDataAvailable("All timeSeries entries were empty"));
}
```

**Result:** `parse_iv_response()` returns:
- `Ok(Vec<GaugeReading>)` — Only contains valid measurements
- `Err(NwisError::NoDataAvailable)` — API returned no valid data

### 3. Store Readings (Valid Data Only)
```rust
fn store_readings(client: &mut Client, readings: &[GaugeReading]) -> Result<...> {
    if readings.is_empty() {
        println!("   ℹ️  No readings to store (all stations may be offline)");
        return Ok(());  // No INSERT happens
    }
    
    // Only valid readings reach this point
    for reading in readings {
        client.execute(
            "INSERT INTO usgs_raw.gauge_readings \
             (site_code, parameter_code, reading_time, value, qualifiers) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT DO NOTHING",
            &[&reading.site_code, &reading.parameter_code, 
              &reading.datetime, &reading.value, &reading.qualifier]
        )?;
    }
}
```

**Database Result:**
- `gauge_readings` table contains ONLY actual measurements
- No NULL values in `value` column
- No sentinel values
- No "station offline" placeholder records

---

## Tracking Absence of Data: Monitoring State

### Separate Subsystem for Polling Metadata

The `monitoring_state` table tracks **when we polled** and **what we received**, including failed polls and empty responses.

```rust
// After attempting to fetch data
record_poll_result(
    &mut db,
    "05568500",
    "00060",
    success: true,           // API request succeeded (HTTP 200)
    readings: &readings,     // May be empty Vec if station offline
)?;
```

### What Gets Recorded in monitoring_state

```sql
-- Example: Successful poll but station returned no data
last_poll_attempted = NOW()              -- We tried
last_poll_succeeded = NOW()              -- API responded HTTP 200
last_data_received = '2026-02-18 10:00'  -- UNCHANGED (last time we got data)
latest_reading_time = '2026-02-18 10:00' -- UNCHANGED (still the last valid reading)
consecutive_failures = consecutive_failures + 1  -- Increment
status = 'offline'                       -- Set based on no data received
```

```sql
-- Example: Successful poll with valid data
last_poll_attempted = NOW()              -- We tried
last_poll_succeeded = NOW()              -- API responded
last_data_received = NOW()               -- We got fresh data!
latest_reading_time = '2026-02-19 14:45' -- Timestamp of newest reading
latest_reading_value = 42300.0           -- Value of newest reading
consecutive_failures = 0                 -- Reset on success
status = 'active'                        -- Station healthy
```

### Status Logic in SQL Function

```sql
status = CASE
    WHEN NOT p_poll_succeeded THEN 'offline'        -- API request failed
    WHEN p_readings_count = 0 THEN 'offline'        -- API succeeded but no data
    WHEN v_is_stale THEN 'degraded'                 -- Have data but it's old
    ELSE 'active'                                   -- Fresh data available
END
```

---

## Why This Separation?

### gauge_readings: Clean Time Series Data
✅ **Query Performance**: No filtering needed to find valid measurements  
✅ **Data Integrity**: All values are real measurements  
✅ **Storage Efficiency**: No wasted rows for "no data"  
✅ **Analytics**: Direct use in flood analysis without data cleaning  
✅ **Time Series Tools**: Compatible with standard time series databases/tools  

**Bad Alternative (Don't Do This):**
```sql
-- ❌ ANTI-PATTERN: Storing "no data" as rows
INSERT INTO gauge_readings VALUES (
    '05568500', '00060', NOW(), NULL, NULL  -- Bad!
);

-- ❌ ANTI-PATTERN: Storing sentinel values
INSERT INTO gauge_readings VALUES (
    '05568500', '00060', NOW(), -999999, 'MISSING'  -- Bad!
);
```

**Why This Is Bad:**
- Every query needs `WHERE value IS NOT NULL AND value != -999999`
- Analytics tools can't distinguish real zeros from missing data
- Wasted storage on non-information
- Confuses "station offline" with "river dried up completely"

### monitoring_state: Operational Metadata
✅ **Track Polling Frequency**: When did we last check?  
✅ **Detect Outages**: Station stopped reporting  
✅ **Alert Configuration**: Different thresholds per station  
✅ **Health Dashboard**: Which stations are operational?  
✅ **Historical Analysis**: How often do stations fail?  

---

## Real-World Example: Station Goes Offline

**Timeline:**
```
Feb 19, 10:00 - Last valid reading received and stored
Feb 19, 10:15 - Poll succeeds, but station returns no data
Feb 19, 10:30 - Poll succeeds, still no data
Feb 19, 10:45 - Poll succeeds, still no data
Feb 19, 11:00 - Poll succeeds, still no data
```

**gauge_readings table after outage:**
```sql
SELECT * FROM usgs_raw.gauge_readings 
WHERE site_code = '05568500' 
ORDER BY reading_time DESC 
LIMIT 1;

-- Result:
site_code | parameter_code | reading_time        | value
----------|----------------|---------------------|-------
05568500  | 00060          | 2026-02-19 10:00:00 | 42300.0

-- NO additional rows added during outage
```

**monitoring_state table after outage:**
```sql
SELECT * FROM usgs_raw.monitoring_state 
WHERE site_code = '05568500';

-- Result:
site_code | last_poll_attempted | last_data_received  | latest_reading_time | consecutive_failures | status
----------|---------------------|---------------------|---------------------|---------------------|--------
05568500  | 2026-02-19 11:00:00 | 2026-02-19 10:00:00 | 2026-02-19 10:00:00 | 4                   | offline

-- Records WHEN we polled and that we got NO data
-- latest_reading_time stays at 10:00 (last valid data)
-- consecutive_failures increments each empty poll
```

**Staleness Check:**
```rust
let age_minutes = (now() - latest_reading_time).num_minutes();
// age = 60 minutes (11:00 - 10:00)

if age_minutes > staleness_threshold_minutes {
    // ALERT: Station 05568500 has stale data!
    // Last reading: 60 minutes ago
    // Status: offline (4 consecutive failed polls)
}
```

---

## Summary: Two-Table Architecture

### Table 1: `gauge_readings` — The Science
**Purpose:** Store actual hydrological measurements  
**Contents:** Only valid, verified data points  
**Used For:** Flood analysis, trend detection, historical research  
**Never Contains:** Nulls, sentinels, "no data" markers  

### Table 2: `monitoring_state` — The Operations
**Purpose:** Track system health and data freshness  
**Contents:** Polling metadata, station status, staleness flags  
**Used For:** Alerts, dashboards, outage detection  
**Records:** Successful polls with empty data, failed API requests  

---

## Code Locations

**Parsing (filters invalid data):**
- `src/ingest/usgs.rs` — `parse_iv_response()` and `parse_dv_response()`
- Skips empty arrays with `continue`
- Skips sentinel values with `continue`
- Returns `Err(NoDataAvailable)` if no valid measurements found

**Storage (valid data only):**
- `src/bin/historical_ingest.rs` — `store_readings()`
- Only inserts GaugeReading structs passed to it
- Never creates placeholder records

**Monitoring (tracks absence):**
- `sql/002_monitoring_metadata.sql` — `update_monitoring_state()` function
- `src/monitor/mod.rs` — `record_poll_result()` function
- Records poll attempts even when `readings.len() == 0`
- Increments consecutive_failures
- Sets status = 'offline' when no data received

---

## Testing the Separation

**Query 1: All readings are valid measurements**
```sql
-- Should return TRUE (no NULLs or sentinels in actual data)
SELECT COUNT(*) = 0 FROM usgs_raw.gauge_readings 
WHERE value IS NULL OR value < -999000;
```

**Query 2: Monitoring tracks empty polls**
```sql
-- Should show stations with no recent data but polls still happening
SELECT site_code, 
       last_poll_attempted,
       last_data_received,
       last_poll_attempted - last_data_received AS gap
FROM usgs_raw.monitoring_state
WHERE last_poll_attempted > last_data_received;
```

**Query 3: Offline stations have no recent readings but poll metadata exists**
```sql
-- Stations marked offline should have old latest_reading_time
SELECT m.site_code, m.status, m.latest_reading_time,
       COUNT(r.id) as recent_readings
FROM usgs_raw.monitoring_state m
LEFT JOIN usgs_raw.gauge_readings r 
    ON m.site_code = r.site_code 
    AND r.reading_time > NOW() - INTERVAL '1 hour'
WHERE m.status = 'offline'
GROUP BY m.site_code, m.status, m.latest_reading_time;

-- Should show: status='offline', 0 recent_readings, old latest_reading_time
```
