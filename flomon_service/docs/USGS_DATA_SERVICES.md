# USGS Data Services - Complete Guide

## Overview

USGS provides **three distinct water data services**, each optimized for different time ranges and use cases. FloPro uses all three services appropriately.

---

## 1. IV Service - Instantaneous Values `/nwis/iv/`

**Purpose:** Real-time monitoring with 15-minute resolution

**Time Range:** Last **120 days** only (rolling window)

**Data Resolution:** 15-minute intervals (instantaneous measurements)

**Best For:**
- Real-time flood monitoring
- Current conditions
- Recent trend analysis
- Nowcasting

**API Endpoint:**
```
https://waterservices.usgs.gov/nwis/iv/
```

**FloPro Implementation:**
- **Module:** [src/ingest/usgs.rs](../src/ingest/usgs.rs)
- **Functions:** `build_iv_url()`, `parse_iv_response()`
- **Used by:** Main daemon polling loop (every 15 minutes)

**Example Request:**
```rust
let url = usgs::build_iv_url(
    &["05568500"],              // Kingston Mines
    &["00060", "00065"],        // Discharge + Stage
    "PT3H",                      // Last 3 hours
);
```

**Example URL:**
```
https://waterservices.usgs.gov/nwis/iv/?sites=05568500&parameterCd=00060,00065&period=PT3H&format=json&siteStatus=active
```

**Response Format:** WaterML 2.0 as JSON

**Limitations:**
- ⚠️ **Only last 120 days available**
- ⚠️ Some stations may have gaps or outages
- ⚠️ Data subject to revision (provisional)

---

## 2. DV Service - Daily Values `/nwis/dv/`

**Purpose:** Historical analysis with daily resolution

**Time Range:** **Full period of record** (often 50-100+ years)

**Data Resolution:** Daily mean values

**Best For:**
- Historical flood analysis
- Long-term trends
- Climatological studies
- Backfilling gaps beyond 120 days

**API Endpoint:**
```
https://waterservices.usgs.gov/nwis/dv/
```

**FloPro Implementation:**
- **Module:** [src/ingest/usgs.rs](../src/ingest/usgs.rs)
- **Functions:** `build_dv_url()`, `parse_dv_response()`
- **Used by:** Historical backfill, gap filling beyond 120 days

**Example Request:**
```rust
let url = usgs::build_dv_url(
    &["05568500"],              // Kingston Mines
    &["00060", "00065"],        // Discharge + Stage
    "2020-01-01",               // Start date
    "2020-12-31",               // End date
);
```

**Example URL:**
```
https://waterservices.usgs.gov/nwis/dv/?sites=05568500&parameterCd=00060,00065&startDT=2020-01-01&endDT=2020-12-31&format=json
```

**Response Format:** WaterML 2.0 as JSON (same structure as IV)

**Data Quality:**
- ✅ **Approved/quality-controlled data**
- ✅ Complete historical record
- ✅ Daily mean discharge and stage
- ✅ Suitable for statistical analysis

**When FloPro Uses DV:**
```rust
// From src/daemon.rs:326
fn backfill_daily_values(&mut self, site_code: &str, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<usize, Box<dyn Error>> {
    let url = usgs::build_dv_url(
        &[site_code],
        &["00060", "00065"],
        &start_date_str,
        &end_date_str,
    );
    // ... fetch and parse ...
}
```

**Daemon Logic:**
- If backfill > 120 days: Use DV for older data
- If IV API fails: Fall back to DV
- For gap filling beyond IV window: DV only

---

## 3. Peak Service - Annual Peaks `/nwis/peak/`

**Purpose:** Flood history database

**Time Range:** **Full period of record** (often back to gauge installation, 50-100+ years)

**Data Resolution:** **One record per year** (annual peak discharge and stage)

**Best For:**
- Flood frequency analysis
- Flood stage threshold validation
- Extreme event history
- Return period calculations

**API Endpoint:**
```
https://nwis.waterdata.usgs.gov/{state}/nwis/peak?site_no={site}&agency_cd=USGS&format=rdb
```

**FloPro Implementation:**
- **Module:** [src/ingest/peak_flow.rs](../src/ingest/peak_flow.rs)
- **Functions:** `parse_rdb()`, `identify_flood_events()`
- **Used by:** Historical flood analysis, threshold validation

**Example Request:**
```
https://nwis.waterdata.usgs.gov/il/nwis/peak?site_no=05568500&agency_cd=USGS&format=rdb
```

**Response Format:** **RDB (tab-delimited text)**, NOT JSON

**Example Data:**
```
agency_cd	site_no	peak_dt	peak_tm	peak_va	peak_cd	gage_ht	gage_ht_cd	year_last_pk
USGS	05568500	1941-04-25		96800		24.62		1941
USGS	05568500	2008-06-15		101000		24.68		2008
USGS	05568500	2013-04-24		96800		24.62		2013
USGS	05568500	2019-05-31		94200		24.21		2019
```

**Columns:**
- `peak_dt` - Date of annual peak
- `peak_va` - Peak discharge (cfs)
- `gage_ht` - Peak gauge height (ft)
- `peak_cd` - Qualification codes

**FloPro Usage:**
```rust
use flomon_service::ingest::peak_flow;

let rdb_text = fetch_from_usgs(url)?;
let records = peak_flow::parse_rdb(&rdb_text)?;
let flood_events = peak_flow::identify_flood_events(&records, &thresholds)?;
```

**Station Configuration:**
```toml
# usgs_stations.toml
[[station]]
site_code = "05568500"
name = "Illinois River at Kingston Mines, IL"

[station.peak_flow]
url = "https://nwis.waterdata.usgs.gov/il/nwis/peak?site_no=05568500&agency_cd=USGS&format=rdb"
period_of_record = "1941-2024"
years_available = 84
notable_floods = "2008-09 (101,000 cfs, 24.68'), 2013-04 (96,800 cfs, 24.62'), 2019-05 (94,200 cfs, 24.21')"
```

---

## Service Selection Logic

### Current/Recent Data (Last 4 months):
→ **IV Service** via `build_iv_url(sites, params, "PT120D")`

### Historical Data (Beyond 4 months):
→ **DV Service** via `build_dv_url(sites, params, start_date, end_date)`

### Flood History/Annual Peaks:
→ **Peak Service** via `parse_rdb(fetch_from_url())`

---

## FloPro Daemon Behavior

### Startup Backfill

```rust
// From src/daemon.rs
pub fn backfill_initial_data(&mut self, backfill_days: i64) -> Result<usize, Box<dyn Error>> {
    let iv_days = backfill_days.min(120);  // IV limited to 120 days
    
    // 1. Get recent data via IV (up to 120 days)
    let iv_count = self.backfill_iv_for_site(site_code, iv_days)?;
    
    // 2. If user requested > 120 days, use DV for older data
    if backfill_days > 120 {
        let dv_count = self.backfill_daily_values(
            site_code, 
            deep_history_start, 
            deep_history_end
        )?;
    }
}
```

### Normal Polling (Every 15 minutes)

```rust
// Get just the last 3 hours of data
let url = usgs::build_iv_url(&site_codes, &["00060", "00065"], "PT3H");
let readings = usgs::parse_iv_response(&fetch(url)?)?;
self.store_readings(&readings)?;
```

### Gap Filling

```rust
// If gap > 120 days old, use DV
if gap_age_days > 120 {
    self.backfill_daily_values(site_code, gap_start, gap_end)?;
} else {
    // Use IV for recent gaps
    self.backfill_iv_for_site(site_code, gap_age_days)?;
}
```

---

## Data Quality Differences

| Aspect | IV Service | DV Service | Peak Service |
|--------|-----------|------------|--------------|
| **Resolution** | 15-minute | Daily mean | Annual peak only |
| **Latency** | Real-time (~15 min) | ~24 hours | Annual |
| **Quality** | Provisional | Approved | Approved |
| **Corrections** | Subject to revision | Final | Final |
| **Use Case** | Monitoring | Analysis | Flood history |
| **Time Range** | 120 days | Full record | Full record |
| **Format** | JSON | JSON | RDB (text) |

---

## Common Issues & Solutions

### Issue: "No timeSeries entries in response"

**Possible Causes:**
1. Station equipment failure/maintenance
2. Requesting data beyond 120-day IV window
3. Station decommissioned
4. Typo in site code

**Solutions:**
```rust
// 1. Try DV service instead
let url = usgs::build_dv_url(sites, params, start_date, end_date);

// 2. Check station status on USGS website
// https://waterdata.usgs.gov/nwis/inventory?agency_code=USGS&site_no=05568500

// 3. Verify site is active
// Use &siteStatus=active in URL (already done by build_iv_url)
```

### Issue: Need data from 2020

**Solution:** Use DV service
```rust
let url = usgs::build_dv_url(
    &["05568500"],
    &["00060", "00065"],
    "2020-01-01",
    "2020-12-31",
);
```

### Issue: Want to know all historical floods

**Solution:** Use Peak service
```rust
let url = "https://nwis.waterdata.usgs.gov/il/nwis/peak?site_no=05568500&agency_cd=USGS&format=rdb";
let records = peak_flow::parse_rdb(&fetch(url)?)?;
let floods = peak_flow::identify_flood_events(&records, &thresholds)?;
```

---

## Testing Each Service

### Test IV Service (Current):
```bash
curl "https://waterservices.usgs.gov/nwis/iv/?sites=05568500&parameterCd=00060,00065&period=PT3H&format=json"
```

### Test DV Service (Historical):
```bash
curl "https://waterservices.usgs.gov/nwis/dv/?sites=05568500&parameterCd=00060,00065&startDT=2020-01-01&endDT=2020-12-31&format=json"
```

### Test Peak Service (Flood History):
```bash
curl "https://nwis.waterdata.usgs.gov/il/nwis/peak?site_no=05568500&agency_cd=USGS&format=rdb"
```

---

## Database Storage

All three services store data in the same table:

```sql
CREATE TABLE usgs_raw.gauge_readings (
    id BIGSERIAL PRIMARY KEY,
    site_code VARCHAR(8) NOT NULL,
    measurement_time TIMESTAMPTZ NOT NULL,
    parameter_code VARCHAR(5) NOT NULL,
    value NUMERIC(12, 4) NOT NULL,
    unit VARCHAR(10) NOT NULL,
    qualifiers TEXT[],
    data_source TEXT DEFAULT 'USGS_NWIS',
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT unique_reading UNIQUE (site_code, measurement_time, parameter_code)
);
```

**Data Source tracking:**
- IV data: `data_source = 'USGS_NWIS_IV'` (implicit, or just 'USGS_NWIS')
- DV data: `data_source = 'USGS_NWIS_DV'`
- Peak data: Could be stored in separate table or denormalized

---

## Summary

### When to Use Each Service:

| Need | Service | Method | Time Range |
|------|---------|--------|------------|
| **Real-time flood monitoring** | IV | `build_iv_url(..., "PT3H")` | Last 3 hours |
| **Recent trends** | IV | `build_iv_url(..., "PT30D")` | Last 30 days |
| **Historical analysis 2020** | DV | `build_dv_url(..., "2020-01-01", "2020-12-31")` | Any date range |
| **Flood event history** | Peak | `parse_rdb(fetch(peak_url))` | Full record |
| **Backfill startup** | IV then DV | Daemon handles automatically | 0-120 days: IV, 120+: DV |
| **Gap beyond 4 months** | DV | `backfill_daily_values()` | Historical only |

**FloPro automatically selects the right service** based on the requested time range and data availability.

---

## References

- **USGS Water Services:** https://waterservices.usgs.gov/
- **IV Documentation:** https://waterservices.usgs.gov/docs/instantaneous-values/
- **DV Documentation:** https://waterservices.usgs.gov/docs/daily-values/
- **Peak Flow Database:** https://nwis.waterdata.usgs.gov/nwis/peak
- **Site Inventory:** https://waterdata.usgs.gov/nwis/inventory
