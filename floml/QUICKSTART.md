# FloML Quick Reference

**FloML** (Flood Machine Learning) - Statistical analysis for flood prediction

## Project Structure

```
floml/
├── floml/                      # Main Python package
│   ├── __init__.py            # Package initialization
│   ├── db.py                  # 🔌 Database connections
│   ├── regression.py          # 📊 Segmented linear regression
│   ├── correlation.py         # 🔗 Multi-station correlation
│   └── precursors.py          # ⚠️  Flood precursor detection
│
├── scripts/                    # Standalone analysis scripts
│   └── analyze_events.py      # 🔍 Example event analysis
│
├── notebooks/                  # Jupyter notebooks
│   └── example_analysis.ipynb # 📓 Interactive examples
│
├── tests/                      # Unit tests (TODO)
├── requirements.txt            # Python dependencies
├── .env                        # Database configuration
└── README.md                  # Full documentation
```

## Quick Start

```bash
# 1. Activate virtual environment
cd /home/fiver/projects/flopro/floml
source venv/bin/activate

# 2. Test connection
python floml/db.py
# Output: ✓ Database connected - 8 USGS sites configured

# 3. Run example analysis
python scripts/analyze_events.py --site-code 05568500
```

## Core Modules

### 1. Database (`db.py`)

```python
from floml.db import get_engine, get_connection

# SQLAlchemy engine (for pandas)
engine = get_engine()
data = pd.read_sql("SELECT * FROM usgs_raw.sites", engine)

# Raw psycopg2 connection
conn = get_connection()
```

### 2. Segmented Regression (`regression.py`)

**For non-linear stage-discharge relationships**

```python
from floml.regression import fit_stage_discharge

# Fit 3-segment model
result = fit_stage_discharge(discharge_cfs, stage_ft, n_segments=3)

print(f"R² = {result.r_squared:.4f}")
print(f"Breakpoints: {result.breakpoints}")

# Predict stage from discharge
predicted_stage = result.predict([25000])  # 25,000 cfs
```

**Key features:**
- Automatically finds optimal breakpoints
- Handles channel overflow, floodplain expansion
- Returns R², RMSE, slopes, intercepts

### 3. Multi-Station Correlation (`correlation.py`)

**Analyze upstream-downstream relationships**

```python
from floml.correlation import correlate_stations

# Auto-detect time lag
result = correlate_stations(upstream_stage, downstream_stage)

print(f"Correlation: {result.pearson_r:.3f}")
print(f"Lag: {result.lag_hours} hours")

# Predict downstream value
predicted, lag = predict_downstream(13.5, result)
print(f"If upstream is 13.5 ft now, downstream will be {predicted:.1f} ft in {lag} hours")
```

**Key features:**
- Cross-correlation to find optimal lag
- Pearson correlation and linear regression
- Network analysis for multiple stations

### 4. Precursor Detection (`precursors.py`)

**Identify early warning signals before floods**

```python
from floml.precursors import analyze_precursors, compute_precursor_metrics

# Detect precursors 14 days before peak
precursors = analyze_precursors(stage_series, peak_time, lookback_days=14)

for p in precursors:
    print(f"{p.precursor_type}: {p.hours_before_peak:.1f} hours warning")

# Summary metrics
metrics = compute_precursor_metrics(precursors)
print(f"Earliest warning: {metrics['earliest_warning_hours']:.1f} hours")
```

**Detects:**
- Rapid rise events (>0.5 ft/day)
- Sustained rise over multiple days
- Classifies severity (minor/moderate/major)

## Example Workflows

### Analyze Recent Flood Events

```bash
python scripts/analyze_events.py
```

### Interactive Exploration

```bash
jupyter notebook notebooks/example_analysis.ipynb
```

### Load and Analyze Custom Data

```python
import pandas as pd
from floml.db import get_engine
from floml.regression import fit_segmented_regression

engine = get_engine()

# Load your data
data = pd.read_sql("""
    SELECT value as discharge, other_value as stage
    FROM your_table
""", engine)

# Fit model
model = fit_segmented_regression(
    data['discharge'], 
    data['stage'], 
    n_segments=3
)

print(model)
```

## Database Access Patterns

FloML reads from schemas curated by Rust daemon:

| Schema | Description | Example Tables |
|--------|-------------|----------------|
| `usgs_raw.*` | USGS gauge data | `sites`, `gauge_readings` |
| `nws.*` | NWS flood metadata | `flood_events`, `flood_thresholds` |
| `usace.*` | USACE CWMS data | `cwms_locations`, `cwms_timeseries` |

Write results to:
- `flood_analysis.*` - Your analysis outputs

## Common Queries

**Load stage data for a site:**
```python
data = pd.read_sql("""
    SELECT reading_time, value as stage_ft
    FROM usgs_raw.gauge_readings
    WHERE site_code = '05568500'
      AND parameter_code = '00065'  -- stage
      AND reading_time > NOW() - INTERVAL '30 days'
    ORDER BY reading_time
""", engine)
```

**Load flood events:**
```python
events = pd.read_sql("""
    SELECT e.*, t.flood_stage_ft
    FROM nws.flood_events e
    JOIN nws.flood_thresholds t ON e.site_code = t.site_code
    WHERE e.crest_time IS NOT NULL
""", engine)
```

**Load paired stage-discharge:**
```python
paired = pd.read_sql("""
    SELECT reading_time,
           MAX(CASE WHEN parameter_code = '00065' THEN value END) as stage_ft,
           MAX(CASE WHEN parameter_code = '00060' THEN value END) as discharge_cfs
    FROM usgs_raw.gauge_readings
    WHERE site_code = '05568500'
    GROUP BY reading_time
    HAVING stage_ft IS NOT NULL AND discharge_cfs IS NOT NULL
""", engine)
```

## Dependencies

**Core analysis:**
- `numpy` - Numerical computing
- `pandas` - Data manipulation
- `scipy` - Scientific computing
- `scikit-learn` - Machine learning
- `pwlf` - Piecewise linear fitting (segmented regression)

**Visualization:**
- `matplotlib` - Plotting
- `seaborn` - Statistical graphics

**Development:**
- `jupyter` - Interactive notebooks
- `pytest` - Testing

## Troubleshooting

**"DATABASE_URL not set"**
- Check `.env` file exists
- Format: `postgresql://user:pass@localhost/flopro_db`

**"Missing required database schemas"**
- Run Rust migrations first: `cd ../flomon_service/sql/`
- Apply all `*.sql` files in order

**"No data found"**
- Ingest data: `cargo run --bin historical_ingest`
- Load peak flows: `cargo run --bin ingest_peak_flows`

**Import errors**
- Activate venv: `source venv/bin/activate`
- Reinstall: `pip install -r requirements.txt`

## Next Steps

1. **Ingest more data** (from Rust daemon)
2. **Run example analysis** (`scripts/analyze_events.py`)
3. **Explore in Jupyter** (`notebooks/example_analysis.ipynb`)
4. **Build custom models** for your specific use case
5. **Write results** back to `flood_analysis` schema

---

**Architecture:** Rust daemon curates data → Python analyzes it  
**See also:** `../flomon_service/docs/PYTHON_INTEGRATION.md`
