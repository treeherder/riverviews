## Threshold Management Strategy

### Current: Static NWS Thresholds

The Rust daemon uses **simple static thresholds** from NWS (National Weather Service):

```
Action Stage     → 14.0 ft  (monitor closely)
Flood Stage      → 16.0 ft  (minor flooding begins)
Moderate Flood   → 20.0 ft  (moderate flooding)
Major Flood      → 24.0 ft  (major flooding)
```

These are stored in:
- `usgs_stations.toml` configuration
- `nws.flood_thresholds` database table

### Future: ML-Discovered Thresholds

Python/FloML can **discover better thresholds** through analysis:

1. **Segmented Regression** - Find where slope changes in stage-discharge curve
   - Channel → Bankfull → Floodplain transitions
   - These are natural physical breakpoints

2. **Historical Correlation** - Identify what stage correlates with impacts
   - When does flooding actually start?
   - What stage causes road closures, property damage?

3. **Multi-Station Analysis** - Optimize thresholds based on downstream effects
   - What upstream level reliably predicts downstream flooding?

### Updating Thresholds

FloML analysis can write recommended thresholds to a new table:

```sql
CREATE TABLE flood_analysis.recommended_thresholds (
    site_code VARCHAR(15),
    threshold_type VARCHAR(20),  -- 'warning', 'minor', 'moderate', 'major'
    stage_ft DECIMAL(6,2),
    confidence DECIMAL(4,3),     -- 0.0 to 1.0
    method VARCHAR(50),          -- 'segmented_regression', 'historical_correlation'
    discovered_at TIMESTAMP,
    active BOOLEAN DEFAULT false
);
```

Then daemon can:
- Use ML thresholds when `active = true`
- Fall back to NWS thresholds otherwise
- Compare both sets of thresholds

### Workflow

```
┌─────────────────────────┐
│   Rust Daemon           │
│  ┌──────────────────┐   │
│  │ Current: NWS     │   │
│  │ 16.0 ft = flood  │   │
│  └──────────────────┘   │
│         │               │
│         ▼               │
│   Compare reading       │
│   Generate alert        │
└─────────────────────────┘
         │
         ▼
  ┌─────────────┐
  │ PostgreSQL  │
  │   readings  │
  └──────┬──────┘
         │
         ▼
┌─────────────────────────┐
│  Python/FloML           │
│  ┌──────────────────┐   │
│  │ Analyze curves   │   │
│  │ Discover: 15.3ft │   │
│  │ better threshold │   │
│  └──────────────────┘   │
│         │               │
│         ▼               │
│  Write recommendation   │
└─────────────────────────┘
         │
         ▼
  ┌─────────────────────┐
  │ Update thresholds   │
  │ Daemon uses ML      │
  │ values for alerts   │
  └─────────────────────┘
```

### Example: FloML Threshold Discovery

```python
from floml.regression import fit_stage_discharge
from floml.db import get_engine
import pandas as pd

engine = get_engine()

# Load stage-discharge data
data = pd.read_sql("""
    SELECT reading_time,
           MAX(CASE WHEN parameter_code = '00065' THEN value END) as stage_ft,
           MAX(CASE WHEN parameter_code = '00060' THEN value END) as discharge_cfs
    FROM usgs_raw.gauge_readings
    WHERE site_code = '05568500'
    GROUP BY reading_time
    HAVING stage_ft IS NOT NULL AND discharge_cfs IS NOT NULL
""", engine)

# Fit segmented regression (3 segments = 2 breakpoints)
result = fit_stage_discharge(data['discharge_cfs'], data['stage_ft'], n_segments=3)

# Breakpoints are natural thresholds (channel → bankfull → floodplain)
print(f"Discovered breakpoints: {result.breakpoints}")
# e.g., [1000, 15.3, 19.8, 50000]
#        min   ↑warning ↑major   max

# Store recommendation
with engine.connect() as conn:
    conn.execute("""
        INSERT INTO flood_analysis.recommended_thresholds
        (site_code, threshold_type, stage_ft, confidence, method, discovered_at)
        VALUES ('05568500', 'warning', 15.3, 0.95, 'segmented_regression', NOW())
    """)
```

### Benefits

1. **Immediate Protection** - NWS thresholds work day one
2. **Continuous Improvement** - ML refines thresholds over time
3. **Site-Specific** - Thresholds optimized for local conditions
4. **Explainable** - Physical breakpoints (not black-box ML)
5. **Validated** - Compare NWS vs ML thresholds, choose best

### Implementation Status

- ✅ Rust daemon: Simple threshold checking implemented
- ✅ Python/FloML: Segmented regression ready
- ⏸️ Threshold update mechanism (future)
- ⏸️ Dual threshold comparison (future)
