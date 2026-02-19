# Staleness Tracking

FloPro implements a **hybrid database + in-memory architecture** to ensure data freshness without constant database queries.

## The Staleness Problem

### Why It Matters

During flood events, **stale data is dangerous**:
- Gauge failure might not be obvious from dashboard display
- Flood could be worsening while we show outdated "safe" levels
- False sense of security if last reading was 2 hours ago

### USGS Data Characteristics

**Normal Behavior:**
- New readings every **15 minutes** (00, 15, 30, 45 of each hour)
- Typical API lag: 15-30 minutes
- Expected data age: Usually < 30 minutes

**Staleness Thresholds:**
- **20 minutes** - Critical stations (Kingston Mines, Peoria, Chillicothe)
- **60 minutes** - Normal monitoring sites
- **> 60 minutes** - Consider station offline or degraded

## Architectural Decision: Hybrid Approach

### Two-Tier System

```
┌─────────────────────────────────────┐
│      PostgreSQL Database            │  ← Source of truth
│  ┌──────────────────────────────┐   │     (persistent, auditable)
│  │  monitoring_state table      │   │
│  │  - last_poll_attempted       │   │
│  │  - last_data_received        │   │
│  │  - latest_reading_time       │   │
│  │  - consecutive_failures      │   │
│  │  - status                    │   │
│  └──────────────────────────────┘   │
└──────────────┬──────────────────────┘
               │ Refresh on startup
               │ + after each poll
               ▼
┌─────────────────────────────────────┐
│    In-Memory Cache (HashMap)        │  ← Performance
│  ┌──────────────────────────────┐   │     (fast staleness checks)
│  │  MonitoringCache struct      │   │
│  │  Key: (site, param)          │   │
│  │  Value: StationCache         │   │
│  └──────────────────────────────┘   │
└─────────────────────────────────────┘
```

### Why Hybrid?

**Database Layer** (persistence):
- ✅ Survives service restarts
- ✅ Queryable for dashboards
- ✅ Historical tracking of outages
- ✅ ACID guarantees
- ❌ Too slow for every reading check

**In-Memory Cache** (performance):
- ✅ Fast HashMap lookup (O(1))
- ✅ No DB round-trip on hot path
- ✅ Simple to refresh
- ❌ Lost on restart (reload from DB)

**Alternatives Rejected:**

❌ **Pure Database** - Too slow for per-reading checks  
❌ **Pure In-Memory** - Lost on crash, no audit trail  
❌ **Disk State Files** - Duplicates DB, sync complexity  

## Database Schema

### monitoring_state Table

```sql
CREATE TABLE usgs_raw.monitoring_state (
    site_code VARCHAR(8),
    parameter_code VARCHAR(5),
    
    -- Polling timestamps
    last_poll_attempted TIMESTAMPTZ,      -- Last time we queried API
    last_poll_succeeded TIMESTAMPTZ,      -- Last successful API response
    last_data_received TIMESTAMPTZ,       -- Last time we got fresh readings
    
    -- Latest valid reading
    latest_reading_time TIMESTAMPTZ,      -- Timestamp of most recent measurement
    latest_reading_value NUMERIC(12, 4),  -- Value of most recent measurement
    
    -- Failure tracking
    consecutive_failures INTEGER DEFAULT 0,  -- Reset to 0 on success
    status VARCHAR(20) DEFAULT 'active',     -- active | degraded | offline
    status_since TIMESTAMPTZ DEFAULT NOW(),  -- When status last changed
    
    -- Staleness configuration
    is_stale BOOLEAN DEFAULT false,
    stale_since TIMESTAMPTZ,
    staleness_threshold_minutes INTEGER DEFAULT 60,  -- Per-station threshold
    
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (site_code, parameter_code)
);
```

### Station Status State Machine

```
┌────────┐  Data age > threshold   ┌───────────┐
│ active ├────────────────────────>│ degraded  │
└───┬────┘                         └─────┬─────┘
    │                                    │
    │ Fresh data received                │ Poll returns no data
    │                                    │
    │  ┌─────────────────────────────────┘
    │  │                                  
    ▼  ▼                              ┌──────────┐
    ────────────────────────────────> │ offline  │
         Fresh data received          └──────────┘
```

**Status Definitions:**
- **active**: Latest reading within staleness threshold (fresh data)
- **degraded**: Latest reading exceeds threshold but station reporting
- **offline**: No data in recent polls (API returns empty or errors)

### update_monitoring_state() Function

**Purpose:** Atomic update of monitoring metadata after each poll

```sql
CREATE OR REPLACE FUNCTION usgs_raw.update_monitoring_state(
    p_site_code VARCHAR(8),
    p_parameter_code VARCHAR(5),
    p_poll_succeeded BOOLEAN,
    p_readings_count INTEGER,
    p_latest_reading_time TIMESTAMPTZ DEFAULT NULL,
    p_latest_reading_value NUMERIC DEFAULT NULL
)
RETURNS void AS $$
DECLARE
    v_threshold_minutes INTEGER;
    v_reading_age_minutes INTEGER;
    v_is_stale BOOLEAN;
    v_old_status VARCHAR(20);
BEGIN
    -- Get current threshold and status
    SELECT staleness_threshold_minutes, status
    INTO v_threshold_minutes, v_old_status
    FROM usgs_raw.monitoring_state
    WHERE site_code = p_site_code AND parameter_code = p_parameter_code;
    
    -- Calculate staleness
    IF p_latest_reading_time IS NOT NULL THEN
        v_reading_age_minutes := EXTRACT(EPOCH FROM (NOW() - p_latest_reading_time)) / 60;
        v_is_stale := v_reading_age_minutes > v_threshold_minutes;
    ELSE
        v_is_stale := true;
    END IF;
    
    -- Update monitoring state
    UPDATE usgs_raw.monitoring_state
    SET
        last_poll_attempted = NOW(),
        last_poll_succeeded = CASE WHEN p_poll_succeeded THEN NOW() ELSE last_poll_succeeded END,
        last_data_received = CASE WHEN p_readings_count > 0 THEN NOW() ELSE last_data_received END,
        latest_reading_time = COALESCE(p_latest_reading_time, latest_reading_time),
        latest_reading_value = COALESCE(p_latest_reading_value, latest_reading_value),
        consecutive_failures = CASE 
            WHEN p_poll_succeeded AND p_readings_count > 0 THEN 0
            ELSE consecutive_failures + 1
        END,
        status = CASE
            WHEN NOT p_poll_succeeded THEN 'offline'
            WHEN p_readings_count = 0 THEN 'offline'
            WHEN v_is_stale THEN 'degraded'
            ELSE 'active'
        END,
        is_stale = v_is_stale,
        updated_at = NOW()
    WHERE site_code = p_site_code AND parameter_code = p_parameter_code;
END;
$$ LANGUAGE plpgsql;
```

### station_health View

**Purpose:** Dashboard-ready view of all station states

```sql
CREATE OR REPLACE VIEW usgs_raw.station_health AS
SELECT 
    ms.site_code,
    s.site_name,
    ms.parameter_code,
    ms.status,
    ms.status_since,
    ms.is_stale,
    ms.latest_reading_time,
    EXTRACT(EPOCH FROM (NOW() - ms.latest_reading_time)) / 60 AS age_minutes,
    ms.staleness_threshold_minutes,
    ms.consecutive_failures,
    ms.last_poll_attempted,
    ms.last_poll_succeeded
FROM usgs_raw.monitoring_state ms
JOIN usgs_raw.sites s ON ms.site_code = s.site_code
WHERE s.active = true
ORDER BY 
    CASE ms.status
        WHEN 'offline' THEN 1
        WHEN 'degraded' THEN 2
        WHEN 'active' THEN 3
    END,
    s.site_code;
```

**Usage:**
```sql
-- Quick health check
SELECT site_name, status, age_minutes 
FROM usgs_raw.station_health 
WHERE status != 'active';
```

## Rust Implementation

### In-Memory Cache Structure

```rust
// src/monitor/mod.rs

pub struct MonitoringCache {
    cache: HashMap<(String, String), StationCache>,
    last_refresh: DateTime<Utc>,
}

pub struct StationCache {
    pub site_code: String,
    pub parameter_code: String,
    pub latest_reading_time: Option<DateTime<Utc>>,
    pub latest_reading_value: Option<f64>,
    pub staleness_threshold_minutes: i32,
    pub status: StationStatus,
    pub last_poll_attempted: Option<DateTime<Utc>>,
}

pub enum StationStatus {
    Active,
    Degraded,
    Offline,
    Unknown,
}
```

### Refresh from Database

```rust
impl MonitoringCache {
    pub fn refresh_from_db(&mut self, client: &mut Client) -> Result<...> {
        let rows = client.query(
            "SELECT site_code, parameter_code, latest_reading_time, 
                    latest_reading_value, staleness_threshold_minutes, 
                    status, last_poll_attempted 
             FROM usgs_raw.monitoring_state",
            &[],
        )?;

        self.cache.clear();

        for row in rows {
            let cache_entry = StationCache {
                site_code: row.get(0),
                parameter_code: row.get(1),
                latest_reading_time: row.get(2),
                // ... other fields
            };
            
            self.cache.insert(
                (cache_entry.site_code.clone(), cache_entry.parameter_code.clone()),
                cache_entry
            );
        }

        self.last_refresh = Utc::now();
        Ok(())
    }
}
```

### Fast Staleness Check

```rust
impl MonitoringCache {
    pub fn is_stale(&self, site_code: &str, parameter_code: &str, now: DateTime<Utc>) -> bool {
        if let Some(cached) = self.get(site_code, parameter_code) {
            if let Some(reading_time) = cached.latest_reading_time {
                let age_minutes = (now - reading_time).num_minutes();
                return age_minutes > cached.staleness_threshold_minutes as i64;
            }
        }
        true // Unknown stations considered stale by default
    }
    
    pub fn unhealthy_stations(&self) -> Vec<&StationCache> {
        self.cache
            .values()
            .filter(|s| s.status == StationStatus::Offline || s.status == StationStatus::Degraded)
            .collect()
    }
}
```

### Recording Poll Results

```rust
pub fn record_poll_result(
    client: &mut Client,
    site_code: &str,
    parameter_code: &str,
    success: bool,
    readings: &[GaugeReading],
) -> Result<...> {
    // Find latest reading for this site/parameter
    let latest = readings
        .iter()
        .filter(|r| r.site_code == site_code && r.parameter_code == parameter_code)
        .max_by_key(|r| &r.datetime);

    let (latest_time, latest_value) = if let Some(reading) = latest {
        let dt = chrono::DateTime::parse_from_rfc3339(&reading.datetime)
            .map(|dt| dt.with_timezone(&Utc))
            .ok();
        (dt, Some(reading.value))
    } else {
        (None, None)  // No readings for this station
    };

    // Update database
    client.execute(
        "SELECT usgs_raw.update_monitoring_state($1, $2, $3, $4, $5, $6)",
        &[
            &site_code,
            &parameter_code,
            &success,
            &(readings.len() as i32),
            &latest_time,
            &latest_value,
        ],
    )?;

    Ok(())
}
```

## Data Flow Example

### Scenario: Station Goes Offline

**Timeline:**
```
10:00 AM - Last valid reading received (discharge = 42,300 cfs)
10:15 AM - Poll API, get empty response
10:30 AM - Poll API, still empty
10:45 AM - Poll API, still empty
11:00 AM - Poll API, still empty
```

**After Each Poll:**

**10:15 AM Poll (first failure):**
```rust
record_poll_result(
    db,
    "05568500",
    "00060",
    success: true,    // API responded HTTP 200
    readings: &[]     // But returned no data
);
```

**Database State:**
```sql
monitoring_state:
  last_poll_attempted:  2026-02-19 10:15:00
  last_poll_succeeded:  2026-02-19 10:15:00
  last_data_received:   2026-02-19 10:00:00  -- UNCHANGED
  latest_reading_time:  2026-02-19 10:00:00  -- UNCHANGED
  consecutive_failures: 1                     -- INCREMENTED
  status:               'offline'             -- CHANGED
  is_stale:             false                 -- Still within 20-min threshold
```

**11:00 AM Poll (fourth failure):**

**Database State:**
```sql
monitoring_state:
  last_poll_attempted:  2026-02-19 11:00:00
  last_data_received:   2026-02-19 10:00:00  -- Still old
  latest_reading_time:  2026-02-19 10:00:00  -- Still old
  consecutive_failures: 4
  status:               'offline'
  is_stale:             true                  -- NOW TRUE (60 min > 20 min threshold)
  stale_since:          2026-02-19 10:20:00  -- When it became stale
```

**In-Memory Cache:**
```rust
cache.is_stale("05568500", "00060", Utc::now()) 
// Returns: true
// Reason: (11:00 - 10:00) = 60 minutes > 20 minute threshold

cache.unhealthy_stations()
// Returns: [StationCache { site_code: "05568500", status: Offline, ... }]
```

### Scenario: Station Recovers

**12:00 PM - Fresh Data Received:**

```rust
record_poll_result(
    db,
    "05568500",
    "00060",
    success: true,
    readings: &[GaugeReading { 
        datetime: "2026-02-19T12:00:00-06:00",
        value: 43100.0,
        ...
    }]
);
```

**Database State:**
```sql
monitoring_state:
  last_poll_attempted:  2026-02-19 12:00:00
  last_poll_succeeded:  2026-02-19 12:00:00
  last_data_received:   2026-02-19 12:00:00  -- UPDATED
  latest_reading_time:  2026-02-19 12:00:00  -- UPDATED
  latest_reading_value: 43100.0               -- UPDATED
  consecutive_failures: 0                     -- RESET
  status:               'active'              -- RECOVERED
  is_stale:             false
  stale_since:          NULL
```

## Monitoring Service Loop

### Integration Example

```rust
// Simplified real-time monitoring loop (future main.rs)

let mut cache = MonitoringCache::new();
cache.refresh_from_db(&mut db)?;

loop {
    // 1. Poll USGS API for all stations
    let readings = fetch_all_stations(&sites)?;
    
    // 2. Store valid readings in database
    store_readings(&mut db, &readings)?;
    
    // 3. Update monitoring state for each station
    for station in &sites {
        for param in &[PARAM_DISCHARGE, PARAM_STAGE] {
            record_poll_result(
                &mut db,
                &station.site_code,
                param,
                true,  // Assuming API succeeded
                &readings
            )?;
        }
    }
    
    // 4. Refresh in-memory cache
    cache.refresh_from_db(&mut db)?;
    
    // 5. Check for alerts
    let unhealthy = cache.unhealthy_stations();
    if !unhealthy.is_empty() {
        send_staleness_alerts(&unhealthy)?;
    }
    
    // 6. Sleep until next poll
    std::thread::sleep(Duration::from_secs(15 * 60));
}
```

## Testing Staleness Logic

### Unit Tests (Deterministic)

```rust
#[test]
fn test_cache_staleness_check() {
    let mut cache = MonitoringCache::new();
    
    let station = StationCache {
        latest_reading_time: Some(Utc::now() - Duration::minutes(90)),
        staleness_threshold_minutes: 60,
        ...
    };
    
    cache.cache.insert(("05568500".into(), "00060".into()), station);

    assert!(cache.is_stale("05568500", "00060", Utc::now()));
    // 90 minutes > 60 minute threshold = stale
}
```

### Integration Tests (Live Database)

```sql
-- Verify monitoring state tracks polls
SELECT COUNT(*) FROM usgs_raw.monitoring_state;
-- Should equal: 8 sites × 2 parameters = 16 rows

-- Find currently stale stations
SELECT site_code, parameter_code, age_minutes 
FROM usgs_raw.station_health 
WHERE is_stale = true;
```

## Performance Characteristics

### Cache Lookups

**Complexity:** O(1) HashMap lookup  
**Typical Time:** < 1 microsecond  
**Memory:** ~100 bytes per station (16 stations = 1.6 KB)

### Database Refresh

**Frequency:** Every 15 minutes + on startup  
**Query Time:** < 10ms (16 rows)  
**Network:** Single round-trip

### Comparison: Cache vs DB Query

```
Checking staleness for 100 readings:

WITH CACHE:
  100 × 1 μs = 100 μs = 0.1 ms

WITHOUT CACHE (direct DB):
  100 × 5 ms = 500 ms = 0.5 seconds
  (5000× slower)
```

---

**Related Pages:**
- [[Database Architecture]] - monitoring_state table schema
- [[Data Sources]] - USGS update frequency
- [[Technology Stack]] - Why hybrid approach
- [[Real-Time Monitoring]] - Integration in main service loop
