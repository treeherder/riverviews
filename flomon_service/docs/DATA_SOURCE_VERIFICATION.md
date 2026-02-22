# Data Source Verification Framework

This framework provides automated testing of all configured data sources to determine what actually works before investing time in integration.

## Quick Start

```bash
# Run verification via CLI
./target/release/flomon_service verify

# Run via integration tests
cargo test --test data_source_verification -- --nocapture

# Generate markdown report
cargo test test_generate_markdown_report -- --nocapture
```

## What It Does

The verification framework:

1. **Tests each USGS station** - Checks if site exists, has expected parameters, returns data, and has peak flow records
2. **Tests each CWMS location** - Queries catalog API, discovers timeseries, attempts to fetch sample data
3. **Tests each ASOS station** - Verifies API responds, checks data availability, identifies which data types are populated

## Results (as of Feb 22, 2026)

### ✅ ASOS Weather Stations: 100% Working (6/6)
All configured ASOS stations are returning data:
- Temperature, precipitation, wind, pressure all available
- 12-60 observations per station over 4-hour test period
- **Ready for production use**

### ⚠️ USGS Stream Gauges: 62.5% Working (5/8)
Working stations:
- 05568500 - Kingston Mines (primary reference)
- 05567500 - Peoria pool gauge
- 05568000 - Chillicothe
- 05552500 - Marseilles
- 05570000 - Spoon River at Seville

Not working (USGS equipment issues):
- 05557000 - Henry
- 05568580 - Mackinaw River
- 05536890 - Chicago Sanitary Canal at Romeoville

### ❌ CWMS Lock/Dam Data: 0% Working (0/10)
- Illinois River locks (Peoria, Starved Rock, Marseilles, Dresden Island, Brandon Road, Lockport, LaGrange) **not in CWMS API catalog**
- Grafton (Mississippi River) found in catalog but returns no current data (forecast timeseries only)
- Alternative data source needed: `https://rivergages.mvr.usace.army.mil/` may have this data

## Architecture

### Separate Verification Module
Located at `src/verify.rs`, this module is independent of the main daemon and can be run without database setup or configuration.

### Integration Test Suite
Tests in `tests/data_source_verification.rs` provide:
- Individual tests for each data source type
- Detailed diagnostic output
- JSON and Markdown report generation
- Automated validation of configuration files

### CLI Command
The main binary now supports `verify` subcommand for quick validation:
```bash
flomon_service verify
```

## Output Formats

### Console Output
Real-time verification with status indicators:
- ✓ - Success (all expected data available)
- ⚠ - Partial (found but missing some data)  
- ✗ - Failed (not found or no data)

### JSON Report (`verification_report.json`)
Machine-readable with complete details:
- Timestamp
- Per-source results with error messages
- Summary statistics
- Discovered timeseries/parameters

### Markdown Report (`VERIFICATION_REPORT.md`)
Human-readable summary tables showing status of all configured sources.

## Next Steps

Based on verification results:

1. **Focus on ASOS integration first** - 100% working, ready to use
2. **USGS is mostly working** - 5 of 8 stations sufficient for monitoring
3. **CWMS needs alternative approach**:
   - Investigate `rivergages.mvr.usace.army.mil` API
   - Consider NWS river gauge data as alternative
   - May need to use USGS sites colocated with locks instead

## Design Benefits

- **Test before you build** - Validates configuration against live APIs
- **Rapid iteration** - Quick feedback on which data sources are viable
- **CI/CD ready** - Can be run in automated testing pipelines
- **Documentation** - Auto-generates reports showing current system status
- **Isolated module** - Doesn't require full daemon initialization

## Usage in Development

Before adding a new data source:
1. Add configuration to TOML file
2. Run `cargo test test_full_verification_report`
3. Check report to see if source returns data
4. Only implement integration if verification passes

This saves significant development time by identifying non-viable data sources early.
