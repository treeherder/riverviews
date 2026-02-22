# Logging and Error Handling

## Overview

The Flood Monitoring Service uses structured logging to provide context-rich diagnostic information about data source operations. All log messages include:

- **Timestamp** (in log files)
- **Log Level** (DEBUG, INFO, WARN, ERROR)
- **Data Source** (USGS, CWMS, ASOS, DB, SYS)
- **Site/Location Identifier** (when applicable)
- **Failure Classification** (EXPECTED, UNEXPECTED, UNKNOWN)
- **Detailed Error Message**

## Log Outputs

The daemon writes to two destinations:

1. **Console (stdout/stderr)** - Clean, emoji-enhanced output for interactive monitoring
2. **Log File** (`flomon_service.log`) - Timestamped, machine-parseable records

### Log File Location

By default: `./flomon_service.log` in the working directory

To change the location, modify `main.rs`:
```rust
let log_file = "/var/log/flomon/service.log";  // Custom path
logging::init_logger(log_level, Some(log_file), console_timestamps);
```

### Log Levels

| Level | Purpose | Example |
|-------|---------|---------|
| **DEBUG** | Verbose diagnostic info | Expected failures from known-offline stations |
| **INFO** | Normal operations | Successful data ingestion, backfill completion |
| **WARN** | Degraded but functional | Partial failures, unknown failure types |
| **ERROR** | Service degradation | Unexpected API errors, database failures |

Change log level in `main.rs`:
```rust
let log_level = LogLevel::Debug;  // For verbose output
let log_level = LogLevel::Info;   // For normal operations (default)
let log_level = LogLevel::Warning; // For errors only
```

## Failure Classification

The logging system automatically classifies failures to help diagnose issues:

### EXPECTED Failures

**Characteristics:**
- Station is known to be offline, decommissioned, or seasonal
- Listed in a maintenance window
- Part of historical configuration (e.g., old station codes)

**Examples:**
```
2026-02-21 14:32:05 UTC DEBUG USGS [05568500]: IV backfill failed [EXPECTED]: No data available
```

**Action Required:** None - this is normal behavior

### UNEXPECTED Failures

**Characteristics:**
- HTTP errors (500, 503, timeout)
- Parse errors (API format changed)
- Database connection failures
- Authentication errors

**Examples:**
```
2026-02-21 14:32:05 UTC ERROR USGS [05568500]: IV backfill failed [UNEXPECTED]: HTTP error: 503
2026-02-21 14:32:05 UTC ERROR CWMS [LTRNG]: Poll failed [UNEXPECTED]: Connection timeout
```

**Action Required:** Investigate immediately - indicates service degradation

### UNKNOWN Failures

**Characteristics:**
- Empty response from API (could be temporary or permanent)
- No timeSeries data (station may be offline or not reporting parameters)
- Sentinel values (-999999) from USGS

**Examples:**
```
2026-02-21 14:32:05 UTC WARN USGS [05568500]: DV API parsing failed [UNKNOWN]: No timeSeries entries in response
```

**Action Required:** 
- If persistent (>24 hours): check USGS site status
- If affects all stations: check API connectivity
- If affects one station: may be offline for maintenance

## Common Failure Scenarios

### Scenario 1: All Stations Returning No Data

**Example Output:**
```
📥 Backfilling 8 USGS stations...
   ⚠ USGS [05568500]: DV API parsing failed [UNKNOWN]: No timeSeries entries in response
   ✓ 05568500 - Inserted 0 readings
   ⚠ USGS [05567500]: DV API parsing failed [UNKNOWN]: No timeSeries entries in response
   ✓ 05567500 - Inserted 0 readings
   ... (all 8 stations fail)
```

**Likely Causes:**
1. **Future date simulation** - System clock is set to a future date (e.g., 2026) and USGS doesn't have data yet
2. **API outage** - USGS National Water Information System is down
3. **Network connectivity** - Cannot reach waterservices.usgs.gov
4. **Date range issue** - Requesting data from an invalid time period

**Diagnosis:**
```bash
# Check system date
date

# Test USGS API directly
curl "https://waterservices.usgs.gov/nwis/iv/?sites=05568500&parameterCd=00060,00065&period=PT1H&format=json" | jq '.value.timeSeries | length'

# Check network connectivity
ping waterservices.usgs.gov
```

**Resolution:**
- If future date: Correct system clock or use historical date range for testing
- If API outage: Wait for USGS service restoration (check https://waterdata.usgs.gov/nwis/inventory)
- If network issue: Fix connectivity or proxy configuration

### Scenario 2: Single Station Offline

**Example Output:**
```
📥 Backfilling 8 USGS stations...
   ✓ 05568500 - Inserted 2,304 readings
   ✓ 05567500 - Inserted 2,301 readings
   ⚠ USGS [05557000]: IV backfill failed [UNKNOWN]: All timeSeries entries were empty or contained sentinel values
   ✓ 05557000 - Inserted 0 readings
   ... (other stations succeed)
```

**Likely Causes:**
1. **Equipment maintenance** - Gauge offline for repair/calibration
2. **Extreme conditions** - Gauge damaged by flood, ice, or debris
3. **Decommissioned** - Station permanently shut down by USGS

**Diagnosis:**
```bash
# Check USGS site status page
# https://waterdata.usgs.gov/nwis/inventory?site_no=05557000

# View historical availability
# https://nwis.waterdata.usgs.gov/il/nwis/uv?site_no=05557000

# Check last successful reading in database
psql -U flopro_admin -d flopro_db -c \
  "SELECT MAX(reading_time) FROM usgs_raw.gauge_readings WHERE site_code='05557000'"
```

**Resolution:**
- **Temporary outage** (<48 hours): Service will auto-recover and backfill when station returns
- **Extended outage** (>48 hours): Monitor USGS site status page for restoration timeline
- **Permanent decommission**: Remove from `usgs_stations.toml` and document in changelog

### Scenario 3: CWMS Location Returns No Timeseries

**Example Output:**
```
🔍 Discovering CWMS timeseries IDs from catalog...
   Peoria Lock & Dam ... ✓
   LaGrange Lock & Dam ... ✗ No timeseries found matching expected pattern
      Warning: Will skip polling for LaGrange Lock & Dam
```

**Likely Causes:**
1. **Timeseries ID pattern mismatch** - USACE changed naming convention
2. **Location ID typo** - Wrong location code in `usace_iem.toml`
3. **Office parameter incorrect** - Wrong USACE office identifier

**Diagnosis:**
```bash
# Check CWMS catalog directly
curl "https://cwms-data.usace.army.mil/cwms-data/catalog/TIMESERIES?office=MVR&like=LTRGN.*"

# Verify location ID
grep -A 5 "LaGrange" flomon_service/usace_iem.toml
```

**Resolution:**
- Update `usace_iem.toml` with correct location ID or timeseries pattern
- Verify office identifier matches USACE catalog
- Check CWMS API documentation for any recent changes

### Scenario 4: ASOS Station Missing Precipitation Data

**Example Output:**
```
📡 Polling ASOS stations for precipitation...
   ⚠ ASOS [KPIA]: IEM current conditions failed [UNKNOWN]: No precip_1hr_in field in response
   ✓ KPIA - Inserted 0 readings
```

**Likely Causes:**
1. **Sensor offline** - Precipitation gauge not reporting
2. **No precipitation** - IEM API doesn't include field when value is 0.00
3. **API format change** - IEM changed response structure

**Diagnosis:**
```bash
# Check IEM current conditions directly
curl "https://mesonet.agron.iastate.edu/json/current.py?station=KPIA&network=IL_ASOS" | jq '.

# Check 1-minute archive
curl "https://mesonet.agron.iastate.edu/api/1/minute.json?station=KPIA&network=IL_ASOS" | jq '.'
```

**Resolution:**
- If sensor offline: Monitor IEM site status
- If API change: Update `ingest/iem.rs` parser to handle new format
- If no precipitation is normal: Modify parser to default to 0.00 when field missing

## Log File Analysis

### Viewing Recent Errors

```bash
# Last 50 error/warning messages
grep -E "(ERROR|WARN)" flomon_service.log | tail -50

# Errors for specific site
grep "05568500" flomon_service.log | grep ERROR

# Failures in last hour
grep "$(date -u +%Y-%m-%d\ %H)" flomon_service.log | grep -E "(ERROR|WARN)"
```

### Counting Failure Rates

```bash
# Total USGS failures today
grep "$(date -u +%Y-%m-%d)" flomon_service.log | grep "USGS" | grep -c ERROR

# Breakdown by failure type
grep "$(date -u +%Y-%m-%d)" flomon_service.log | grep "USGS" | grep -o "\[.*\]" | sort | uniq -c

# Success rate for last 24 hours
total=$(grep "Backfilling" flomon_service.log | tail -1 | grep -o "[0-9]* USGS" | awk '{print $1}')
failed=$(grep -c "failed \[" flomon_service.log | tail -1)
echo "Success rate: $(( ($total - $failed) * 100 / $total ))%"
```

### Monitoring Trends

```bash
# Group failures by hour (identify outage windows)
awk '/ERROR|WARN/ {print substr($0, 1, 13)}' flomon_service.log | sort | uniq -c

# Most problematic sites
grep -o "\[0-9]\{8\}\]" flomon_service.log | sort | uniq -c | sort -rn | head -10
```

## Integration with Monitoring Tools

### Log Rotation

Prevent unbounded log growth with logrotate:

```bash
# Create /etc/logrotate.d/flomon_service
/var/log/flomon/service.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 0644 flopro flopro
}
```

### Syslog Integration

Forward critical errors to syslog:

Modify `logging.rs` to add:
```rust
use syslog::{Facility, Formatter3164};

// In log() method, add:
if level >= LogLevel::Error {
    let formatter = Formatter3164::init("flomon_service", Facility::LOG_DAEMON);
    syslog::unix(formatter).err(message).ok();
}
```

### Prometheus Metrics (Future Enhancement)

Export failure counters for Prometheus:
```rust
// Track metrics per data source
lazy_static! {
    static ref USGS_FAILURES: IntCounter;
    static ref CWMS_FAILURES: IntCounter;
    static ref ASOS_FAILURES: IntCounter;
}
```

## Debugging Workflow

### 1. Identify the Problem

```bash
# What failed?
tail -100 flomon_service.log | grep ERROR

# How many failed?
grep "Inserted 0 readings" flomon_service.log | tail -10

# Is it persistent?
grep "05568500" flomon_service.log | grep ERROR | tail -5
```

### 2. Classify the Failure

- **All stations fail** → System-level issue (date, network, API)
- **One station fails** → Station-specific issue (offline, decommissioned)
- **One data source fails** → Source-specific issue (API change, credentials)

### 3. Test Externally

```bash
# Test USGS API manually
curl "https://waterservices.usgs.gov/nwis/iv/?sites=05568500&period=PT1H&format=json" | jq '.'

# Test CWMS API
curl "https://cwms-data.usace.army.mil/cwms-data/timeseries?office=MVR&name=LTRGN.Pool.Inst.15Minutes.0.Ccp-Rev&begin=2026-02-20T00:00&end=2026-02-21T00:00" | jq '.'

# Test IEM API
curl "https://mesonet.agron.iastate.edu/json/current.py?station=KPIA&network=IL_ASOS" | jq '.'
```

### 4. Check Database

```bash
# Verify data is (or isn't) being stored
psql -U flopro_admin -d flopro_db << EOF
SELECT site_code, COUNT(*), MAX(reading_time) as latest
FROM usgs_raw.gauge_readings
GROUP BY site_code
ORDER BY latest DESC;
EOF
```

### 5. Adjust Configuration

If station is permanently offline:
```toml
# Comment out in usgs_stations.toml
# [[station]]
# site_code = "05557000"  # Decommissioned 2026-02-15
# name = "Illinois River at Henry, IL"
```

## Expected Behavior by Date

### Current Date (Real-Time Operation)

**API Calls:**
- USGS IV: Request last 1-3 hours (`period=PT3H`)
- USGS DV: Request yesterday and today
- CWMS: Request last 4 hours
- ASOS: Request last 60 minutes

**Expected Results:**
- ✅ All active stations return recent readings
- ⚠️ Seasonal stations may return empty (e.g., ice-affected gauges in winter)
- ⚠️ Provisional data quality (`qualifier: "P"`)

### Historical Backfill

**API Calls:**
- USGS IV: Last 120 days (maximum retention)
- USGS DV: Any date range from 1939-present
- CWMS: Last 120 days (typical)
- ASOS: Last 30 days (IEM 1-minute data)

**Expected Results:**
- ✅ Complete data for operational stations
- ⚠️ Gaps during maintenance windows
- ⚠️ Missing data for decommissioned stations

### Future Dates (Testing/Development)

**API Calls:**
- Any date beyond current system time

**Expected Results:**
- ❌ No data available from any source (future data doesn't exist)
- ⚠️ All stations return empty timeSeries or sentinel values
- 💡 This is **EXPECTED behavior** when system clock is in the future

**Testing Recommendation:**
```bash
# For development/testing, use historical date range
export TEST_DATE_START="2024-01-01"
export TEST_DATE_END="2024-01-31"

# Modify code to use TEST_DATE_* environment variables instead of Utc::now()
```

## Troubleshooting Checklist

- [ ] Check system date: `date -u`
- [ ] Verify network connectivity: `ping waterservices.usgs.gov`
- [ ] Test USGS API manually for one station
- [ ] Check USGS site status: https://waterdata.usgs.gov/nwis/inventory
- [ ] Review last 100 log entries: `tail -100 flomon_service.log`
- [ ] Count failure types: `grep "\[.*\]" flomon_service.log | tail -50`
- [ ] Verify database has recent data: `SELECT MAX(reading_time) FROM usgs_raw.gauge_readings`
- [ ] Check for systematic patterns (all stations vs. one station)
- [ ] Review recent USGS announcements for API changes
- [ ] Confirm environment variables are set: `echo $DATABASE_URL`

## Summary

The logging system is designed to:

1. **Identify Problems** - Clear, site-specific error messages
2. **Classify Severity** - Automatic tagging of expected vs. unexpected failures
3. **Facilitate Diagnosis** - Structured logs with timestamps and context
4. **Support Operations** - Machine-parseable format for monitoring tools
5. **Document Behavior** - Audit trail of all data source operations

Log files are essential for:
- Understanding why stations fail
- Detecting patterns in service degradation
- Verifying successful operations
- Debugging configuration issues
- Meeting compliance/audit requirements

For most **current-date failures showing all stations returning no data**, the root cause is likely:
- **System clock in the future** (most common in development)
- **USGS API outage** (check https://waterdata.usgs.gov/)
- **Network connectivity issue** (test with curl)
