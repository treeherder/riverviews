# Peoria Flood Monitoring Service (FloPro)

A real-time flood monitoring and early warning system for the Peoria, IL region on the Illinois River.

## Overview

FloPro monitors USGS river gauge stations throughout the Illinois River basin to provide early flood warnings for the Peoria area. The service tracks water levels and discharge rates at key upstream monitoring points, detecting flood conditions and providing lead time for flood mitigation and evacuation planning.

### Key Features

- **Real-time monitoring** of USGS gauge stations
- **Multi-site tracking** across the Illinois Waterway
- **Flood stage alerts** based on NWS thresholds
- **Early warning system** with 24-48 hour lead time from upstream stations
- **Data validation** and staleness detection
- **Historical data warehousing** for trend analysis

## Project Structure

```
flomon_service/
├── src/
│   ├── bin/
│   │   └── historical_ingest.rs  # Historical data backfill
│   ├── alert/                     # Flood threshold and staleness checking
│   ├── analysis/                  # Data grouping and trend analysis
│   ├── ingest/                    # USGS API client and data parsing
│   ├── model.rs                   # Core data structures
│   ├── stations.rs                # Station registry and metadata
│   ├── lib.rs                     # Shared library
│   └── main.rs                    # Real-time monitoring service (TBD)
├── tests/                         # Integration tests
└── Cargo.toml
```

---

## Usage

### Historical Data Ingestion

The `historical_ingest` binary populates the database using a **two-tier ingestion strategy**:

**Tier 1: Daily Values (DV) - Long-term Historical Context**
- Data source: USGS Daily Values API  
- Coverage: October 1939 to ~125 days ago
- Resolution: Daily mean discharge and stage
- Use case: Multi-decade flood analysis, seasonal patterns, return intervals

**Tier 2: Instantaneous Values (IV) - Recent High-Resolution**
- Data source: USGS Instantaneous Values API
- Coverage: Last 120 days
- Resolution: 15-minute intervals (96 readings/day)
- Use case: Operational monitoring, short-term trends, flood event detail

This approach provides **87 years of daily historical data** combined with **high-resolution recent data**, enabling both long-term flood risk modeling and operational flood warning.

---

**Initial Run** (populates full historical dataset):
```bash
cd flomon_service
cargo run --bin historical_ingest
```

The binary will execute two phases:
1. **Phase 1 (DV):** Fetch ~87 years of daily data (1939-2026), ~31,755 days per site
2. **Phase 2 (IV):** Fetch 120 days of 15-minute data, ~11,520 readings per site

**Subsequent Runs** (incremental updates):
```bash
cargo run --bin historical_ingest
```
Updates only the recent IV data since last run (DV historical data is static).

---

**State Tracking:**

The binary maintains state in `historical_ingest_state.json`:

```json
{
  "dv_initialized": true,
  "iv_initialized": true,
  "last_update": "2026-02-19T07:15:00.000000000+00:00",
  "dv_last_year_completed": 2025,
  "site_progress": {}
}
```

**Key Features:**
- ✅ **Resumable:** DV ingestion tracks progress by year - can resume if interrupted
- ✅ **Idempotent:** Database `UNIQUE` constraint prevents duplicates
- ✅ **Rate-limited:** 2-second delays between requests to respect USGS API
- ✅ **Two-phase:** Separately tracks DV (historical) and IV (recent) completion

**Configuration:**
- Delete state file to re-run full historical ingestion
- Partial DV ingestion resumes from last completed year
- Edit database directly for manual adjustments

**Expected Ingestion Times** (first run):
```
Phase 1 (DV Historical): ~3-4 minutes
  - 87 years × 8 sites
  - ~2 seconds per year (API rate limit)
  - Total: ~174 seconds + API response time

Phase 2 (IV Recent): ~10-15 seconds
  - 120 days × 8 sites  
  - 1 API call (P120D period)
  - ~11,520 readings per site

Combined first run: ~4-5 minutes total
```

**Storage Estimates:**
- DV data: ~31,755 days × 2 params × 8 sites = ~508,080 daily records
- IV data: ~11,520 readings × 2 params × 8 sites = ~184,320 15-min records
- **Total initial load:** ~692,400 gauge readings

Incremental updates (daily): ~1,152 new IV readings (8 sites × 2 params × 96 readings/day)

**Scheduling:**
Set up a cron job for daily updates:
```cron
# Run daily at 2 AM
0 2 * * * cd /path/to/flomon_service && cargo run --release --bin historical_ingest >> ingest.log 2>&1
```

---

## Data Sources

### USGS NWIS Instantaneous Values (IV) API

The primary data source is the United States Geological Survey's National Water Information System (NWIS) Instantaneous Values service.

**API Endpoint:** `https://waterservices.usgs.gov/nwis/iv/`

#### Data Type & Quality

**Measurements Provided:**
- **Discharge (Parameter 00060)**: Streamflow in cubic feet per second (cfs)
- **Gage Height (Parameter 00065)**: River stage in feet (ft)

**Data Quality Characteristics:**

| Attribute | Value | Notes |
|-----------|-------|-------|
| **Measurement Interval** | 15 minutes | 4 readings per hour at :00, :15, :30, :45 |
| **Data Freshness** | ~15 minutes | Typical lag between measurement and API availability |
| **Data Qualifier** | P (Provisional) or A (Approved) | Provisional data subject to revision |
| **Timezone** | Central Time (CST/CDT) | Timestamps include UTC offset |
| **Coordinate System** | WGS84 (EPSG:4326) | Latitude/longitude in decimal degrees |
| **Missing Data Sentinel** | -999999 | Indicates no data available |

#### Measurement Resolution & Reliability

**Temporal Resolution:**
- New readings every **15 minutes**
- Recommended polling interval: **15-20 minutes**
- Staleness threshold: Data older than **45-60 minutes** indicates potential station outage

**Data Availability:**
- USGS gauges are highly reliable with >95% uptime
- Readings may be temporarily unavailable during:
  - Equipment maintenance
  - Extreme flood events (gauge overflow)
  - Ice conditions
  - Communication failures

**Data Accuracy:**
- Provisional data (qualifier "P"): Subject to revision, ±5-10% accuracy
- Approved data (qualifier "A"): Final QA/QC complete, ±2-5% accuracy
- Stage measurements typically more accurate than discharge calculations

#### API Response Format

Responses are delivered as WaterML rendered in JSON format. Each request returns:
- One `timeSeries` object per site × parameter combination
- Array of time-stamped measurements with qualifiers
- Site metadata (name, location, timezone)
- Parameter metadata (units, description, sentinel values)

**Example:** Request for 1 site with 2 parameters (discharge + stage) returns 2 `timeSeries` entries.

**Key Parsing Notes:**
- ⚠️ Measurement values are **strings** (`"4820"`) not numbers
- Timestamps in ISO 8601 format with timezone offset
- Values array sorted chronologically (most recent = last element)
- Empty value arrays or `-999999` sentinel indicate missing data

#### Data Coverage

**Monitored Stations:**
- Primary: Illinois River at Kingston Mines, IL (05568500)
- Additional stations throughout Peoria basin (see `stations.rs`)
- Upstream early warning points on Illinois Waterway

**Update Frequency:**
- Real-time stations report every 15 minutes
- Data typically available via API within 15-30 minutes of measurement

#### Historical Data Availability & Strategy

**Dual-Source Ingestion:**

FloPro uses two complementary USGS data sources to provide both historical context and operational detail:

| Data Source | API Endpoint | Coverage | Resolution | Records/Site |
|-------------|--------------|----------|------------|--------------|
| **Daily Values (DV)** | `/nwis/dv/` | Oct 1939 - 125 days ago | Daily means | ~31,755 days |
| **Instantaneous Values (IV)** | `/nwis/iv/` | Last 120 days | 15-minute | ~11,520 readings |

**Why Two Sources?**

1. **IV API Limitation:** The Instantaneous Values API maintains only a **120-day rolling window** - data older than ~4 months is archived and unavailable.

2. **DV Long-term Availability:** The Daily Values API provides **87 years of historical data** (1939-present) with daily resolution.

3. **Complementary Coverage:** DV provides flood history for modeling; IV provides operational detail for monitoring.

**Data Availability by Time Period (Site 05568500):**

| Time Period | DV Available | IV Available | Resolution |
|-------------|--------------|--------------|------------|
| 1939-2025 (historical) | ✅ Daily means | ❌ Archived | 1 reading/day |
| Last 120 days | ✅ Daily means | ✅ 15-min | Both available |
| Future updates | ✅ Appended daily | ✅ Rolling window | Ongoing |

**Tested Data Availability (February 2026):**
- **DV API:** Data from October 1939 to present ✅
- **IV API:** Last 120 days only (Nov 2025 - Feb 2026) ✅  
- **DV API beyond 1939:** No data (site established Oct 1939) ❌

---

**For Historical Flood Analysis:**

This dual-ingestion approach enables comprehensive flood risk modeling:

✅ **Decade-scale trends:** Use DV daily data (1939-2026) to identify:
- Historical flood events (1993, 2013, 2019)
- Seasonal patterns and return intervals  
- Long-term climate trends
- Baseline conditions for property risk assessment

✅ **Event-scale detail:** Use IV 15-minute data (<120 days) to analyze:
- Recent flood event progression
- Sub-daily flood dynamics
- Real-time monitoring and alerting
- Validation of flood forecasts

✅ **Continuous coverage:** Database merges both sources:
- 87 years of daily context
- Recent high-resolution overlay
- No data gaps at the 4-month transition point
- Automated updates maintain rolling IV window

**Alternative Sources (Not Currently Implemented):**

The following USGS resources are available for specialized historical analysis:

- **USGS Peak Flow Data:** `https://nwis.waterdata.usgs.gov/nwis/peak`
  - Annual peak discharge records
  - Often extends back 100+ years
  - Critical for flood frequency analysis and return interval calculations
  
- **NWS AHPS Historical Crests:** `https://water.weather.gov/ahps/`
  - Historical high water marks with event narratives
  - Flood stage exceedance records
  - Impact assessments and flood inundation maps

---

## Flood Warning Lead Time

The service leverages upstream monitoring points to provide early flood warnings:

| Station | Location | Lead Time to Peoria |
|---------|----------|---------------------|
| Starved Rock L&D | Near Utica/Ottawa, IL | 24-48 hours |
| Dresden Island L&D | Morris, IL (Illinois River headwaters) | 48-72 hours |
| Brandon Road / Lockport L&D | Joliet, IL | 60-84 hours |

Lead times are approximate and vary with flow velocity, precipitation patterns, and Corps of Engineers operational decisions at locks and dams.

---

## Technology Stack

- **Language:** Rust
- **Data Format:** JSON (WaterML)
- **Dependencies:**
  - `serde` / `serde_json` — JSON parsing
  - `chrono` — Timestamp handling
  
---

## Development Status

**Current Phase:** Historical data warehouse with station resilience

### Completed ✓

- [x] **Station registry and metadata** — 8 USGS gauge stations with parameter tracking
- [x] **USGS IV API client** — Instantaneous Values (15-min, 120-day window)
- [x] **USGS DV API client** — Daily Values (1939-present, 87 years)
- [x] **Historical data ingestion** — Dual-tier DV+IV backfill with resumable state
- [x] **PostgreSQL database** — Multi-schema design with UNIQUE constraints
- [x] **Station resilience** — Graceful degradation, parameter validation, offline handling
- [x] **Integration tests** — Live API verification (manual execution)

### Station Health Status (Verified Feb 19, 2026)

**Operational Stations (6/8):**
- ✅ Illinois River at Kingston Mines, IL (05568500) — discharge + stage
- ✅ Illinois River at Peoria, IL (05567500) — discharge + stage  
- ✅ Illinois River at Chillicothe, IL (05568000) — discharge + stage
- ✅ Spoon River at Seville, IL (05570000) — discharge + stage
- ✅ Illinois River at Marseilles, IL (05552500) — discharge + stage
- ✅ Chicago Sanitary & Ship Canal at Romeoville, IL (05536890) — discharge + stage

**Offline/Decommissioned Stations (2/8):**
- ❌ Illinois River at Henry, IL (05557000) — no data available from API
- ❌ Mackinaw River near Green Valley, IL (05568580) — no data available from API

**System Resilience:**
The service continues operating with 6 active stations. Parse functions skip empty entries, database operations use `ON CONFLICT DO NOTHING` to handle missing data gracefully. See `docs/STATION_RESILIENCE.md` for operational procedures.

**Verification Command:**
```bash
cargo test station_api_verify_all_registry_stations -- --ignored --nocapture
```

### In Progress / Planned

- [ ] **Real-time monitoring service** — Continuous polling and alerting (main.rs)
- [ ] **Data grouping by site** — Multi-site analysis and comparison (9 tests failing)
- [ ] **Flood threshold checking** — NWS action/flood stage alerts (2 tests failing)
- [ ] **Staleness detection** — Identify offline stations and data gaps (9 tests failing)
- [ ] **Alert dispatch system** — Notifications for flood conditions
- [ ] **Web dashboard** — Real-time visualization and historical charts
- [ ] **Station health monitoring** — Automated detection and recovery from outages

---

## License

TBD

---

## Contact & Contribution

This is an experimental flood monitoring service for the Peoria, IL region. 

**Data Sources:**
- USGS NWIS: https://waterservices.usgs.gov/
- NWS AHPS: https://water.noaa.gov/

**Disclaimer:** This service provides informational flood monitoring only. For official flood warnings and emergency information, consult the National Weather Service and local emergency management authorities.
