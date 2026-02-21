# Python Integration Guide

**Purpose:** External Python scripts for complex flood analysis, regression modeling, and statistical computations.

## Architecture

The Rust daemon (`flomon_service`) maintains a curated PostgreSQL database with validated, reliable flood monitoring data. Python scripts read from this database to perform analysis and write results back.

```
┌──────────────────────────────┐
│   Rust Monitoring Daemon     │
│   (Data Curation Layer)      │
│                              │
│  • Ingest from APIs          │
│  • Validate & clean data     │
│  • Track staleness           │
│  • Simple threshold alerts   │
└──────────┬───────────────────┘
           │
           ▼
    ┌──────────────┐
    │  PostgreSQL  │
    │   Database   │
    └──────┬───────┘
           │
    ┌──────┴────────┐
    │               │
    ▼               ▼
┌─────────┐    ┌────────┐
│ Python  │    │ Other  │
│ Scripts │    │ Tools  │
└─────────┘    └────────┘
```

## Database Access

### Read Access (Analysis Input)

Python scripts can read from these schemas:

- **`usgs_raw.*`** - Raw USGS gauge readings
  - `sites` - Station metadata
  - `gauge_readings` - Stage and discharge observations

- **`usace.*`** - USACE CWMS data
  - `cwms_locations` - Mississippi River gauge stations
  - `cwms_timeseries` - Stage, flow, and other parameters

- **`nws.*`** - NWS flood metadata
  - `flood_events` - Historical flood events
  - `flood_thresholds` - Action/flood/moderate/major stages

- **`monitoring.*`** - Service metadata
  - `ingestion_runs` - Data ingestion tracking
  - `staleness_checks` - Data freshness monitoring

### Write Access (Analysis Output)

Python scripts write results to:

- **`flood_analysis.*`** - Analysis results
  - `events` - Analyzed flood events with computed metrics
  - `event_observations` - USGS data linked to events
  - `event_cwms_data` - CWMS data linked to events
  - `event_metrics` - Rise rates, durations, peak stats
  - `precursors` - Detected precursor conditions

Or create new schemas/tables for specialized analysis.

## Connection Setup

### Environment Variables

```bash
# .env file
DATABASE_URL=postgresql://flopro_user:password@localhost/flopro_db
```

### Python Connection Example

Using `psycopg2`:

```python
import os
import psycopg2
from dotenv import load_dotenv

load_dotenv()

def get_connection():
    """Connect to the flood monitoring database."""
    return psycopg2.connect(os.environ['DATABASE_URL'])

# Usage
with get_connection() as conn:
    with conn.cursor() as cur:
        cur.execute("SELECT * FROM usgs_raw.sites WHERE monitored = true")
        sites = cur.fetchall()
```

Using `pandas`:

```python
import os
import pandas as pd
from sqlalchemy import create_engine
from dotenv import load_dotenv

load_dotenv()

engine = create_engine(os.environ['DATABASE_URL'])

# Read data into DataFrame
df = pd.read_sql("""
    SELECT site_code, reading_time, value as stage_ft
    FROM usgs_raw.gauge_readings
    WHERE parameter_code = '00065'
      AND site_code = '05568500'
      AND reading_time > NOW() - INTERVAL '30 days'
    ORDER BY reading_time
""", engine)
```

## Example Analysis Workflows

### 1. Precursor Pattern Detection

```python
"""
Analyze historical flood events to identify precursor patterns.
"""
import pandas as pd
from sqlalchemy import create_engine
import os

engine = create_engine(os.environ['DATABASE_URL'])

# Load historical flood events
events = pd.read_sql("""
    SELECT e.id, e.site_code, e.crest_time, e.peak_stage_ft,
           t.flood_stage_ft
    FROM nws.flood_events e
    JOIN nws.flood_thresholds t ON e.site_code = t.site_code
    WHERE e.crest_time IS NOT NULL
""", engine)

for idx, event in events.iterrows():
    # Get observations for 14 days before peak
    lookback_start = event.crest_time - pd.Timedelta(days=14)
    
    obs = pd.read_sql("""
        SELECT reading_time, value as stage_ft
        FROM usgs_raw.gauge_readings
        WHERE site_code = %s
          AND parameter_code = '00065'
          AND reading_time BETWEEN %s AND %s
        ORDER BY reading_time
    """, engine, params=(event.site_code, lookback_start, event.crest_time))
    
    # Calculate rise rates, detect rapid rise events
    obs['rise_rate_ft_per_day'] = obs['stage_ft'].diff() / \
        (obs['reading_time'].diff().dt.total_seconds() / 86400)
    
    rapid_rise = obs[obs['rise_rate_ft_per_day'] > 0.5]
    
    # Store results
    # ... (insert precursor data into flood_analysis schema)
```

### 2. Multi-Station Regression

```python
"""
Analyze correlation between upstream and downstream stations.
"""
import pandas as pd
import numpy as np
from scipy import stats
from sqlalchemy import create_engine

engine = create_engine(os.environ['DATABASE_URL'])

# Load paired observations from two stations
data = pd.read_sql("""
    SELECT 
        g1.reading_time,
        g1.value as upstream_stage_ft,
        g2.value as downstream_stage_ft
    FROM usgs_raw.gauge_readings g1
    JOIN usgs_raw.gauge_readings g2 
        ON g1.reading_time = g2.reading_time
    WHERE g1.site_code = '05568500'  -- Kingston Mines (upstream)
      AND g2.site_code = '05570000'  -- Peoria (downstream)
      AND g1.parameter_code = '00065'
      AND g2.parameter_code = '00065'
      AND g1.reading_time > NOW() - INTERVAL '1 year'
    ORDER BY g1.reading_time
""", engine)

# Perform linear regression
slope, intercept, r_value, p_value, std_err = stats.linregress(
    data['upstream_stage_ft'], 
    data['downstream_stage_ft']
)

print(f"Correlation coefficient: {r_value:.3f}")
print(f"Downstream = {slope:.3f} * Upstream + {intercept:.2f}")

# Predict downstream stage given upstream conditions
upstream_stage = 20.0  # ft
predicted_downstream = slope * upstream_stage + intercept
print(f"When Kingston Mines is at {upstream_stage} ft, "
      f"Peoria is predicted to be at {predicted_downstream:.2f} ft")
```

### 3. Backwater Influence Analysis

```python
"""
Analyze Mississippi River backwater effects on Illinois River.
"""
import pandas as pd
import numpy as np
from sqlalchemy import create_engine

engine = create_engine(os.environ['DATABASE_URL'])

# Load paired Mississippi and Illinois River data
data = pd.read_sql("""
    SELECT 
        c.timestamp,
        c.value as miss_stage_ft,
        g.value as il_stage_ft
    FROM usace.cwms_timeseries c
    JOIN usace.cwms_locations l ON c.location_id = l.location_id
    CROSS JOIN LATERAL (
        SELECT value, reading_time
        FROM usgs_raw.gauge_readings
        WHERE site_code = '05586100'  -- IL River at Grafton
          AND parameter_code = '00065'
          AND ABS(EXTRACT(EPOCH FROM (reading_time - c.timestamp))) < 3600
        LIMIT 1
    ) g
    WHERE l.location_name LIKE '%Grafton%'
      AND l.river_name = 'Mississippi River'
      AND c.parameter_id = 'Stage'
    ORDER BY c.timestamp
""", engine)

# Calculate differential
data['differential_ft'] = data['miss_stage_ft'] - data['il_stage_ft']

# Identify backwater events (differential > 2 ft)
backwater_periods = data[data['differential_ft'] > 2.0]

# Analyze characteristics
print(f"Backwater conditions detected {len(backwater_periods)} times")
print(f"Mean differential during backwater: {backwater_periods['differential_ft'].mean():.2f} ft")
print(f"Max differential: {data['differential_ft'].max():.2f} ft")
```

## Python Environment Setup

### Recommended Directory Structure

```
flopro/
├── flomon_service/        # Rust daemon
│   ├── src/
│   ├── Cargo.toml
│   └── ...
└── python_analysis/       # Python analysis scripts
    ├── requirements.txt
    ├── .env
    ├── analysis/
    │   ├── __init__.py
    │   ├── precursors.py
    │   ├── regression.py
    │   └── backwater.py
    ├── notebooks/         # Jupyter notebooks for exploration
    │   └── explore_data.ipynb
    └── scripts/           # Standalone analysis scripts
        ├── analyze_events.py
        └── daily_report.py
```

### Dependencies

Create `python_analysis/requirements.txt`:

```
# Database
psycopg2-binary>=2.9.0
sqlalchemy>=2.0.0
python-dotenv>=1.0.0

# Data analysis
pandas>=2.0.0
numpy>=1.24.0
scipy>=1.10.0

# Machine learning (optional)
scikit-learn>=1.3.0

# Visualization (optional)
matplotlib>=3.7.0
seaborn>=0.12.0
plotly>=5.14.0

# Notebooks (optional)
jupyter>=1.0.0
ipython>=8.0.0
```

Install:

```bash
cd python_analysis
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
pip install -r requirements.txt
```

## Best Practices

### 1. Read-Only for Input

Python scripts should treat data in `usgs_raw`, `usace`, `nws`, and `monitoring` schemas as **read-only**. The Rust daemon owns these tables.

### 2. Transaction Management

When writing analysis results:

```python
with conn:  # Auto-commit on success, rollback on error
    with conn.cursor() as cur:
        cur.execute("INSERT INTO flood_analysis.events ...")
```

### 3. Idempotency

Design analysis scripts to be re-runnable:

```python
# Delete and re-compute rather than appending
cur.execute("DELETE FROM flood_analysis.precursors WHERE event_id = %s", (event_id,))
cur.execute("INSERT INTO flood_analysis.precursors ...")
```

Or use `ON CONFLICT` clauses:

```python
cur.execute("""
    INSERT INTO flood_analysis.event_metrics (event_id, rise_rate, ...)
    VALUES (%s, %s, ...)
    ON CONFLICT (event_id) DO UPDATE SET
        rise_rate = EXCLUDED.rise_rate,
        ...
""", (event_id, rise_rate))
```

### 4. Error Handling

```python
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

try:
    # Analysis code
    results = perform_analysis(data)
    store_results(results)
    logger.info("Analysis completed successfully")
except Exception as e:
    logger.error(f"Analysis failed: {e}", exc_info=True)
    raise
```

### 5. Performance

- Use bulk inserts for large result sets
- Create indexes on frequently queried columns
- Use EXPLAIN ANALYZE to optimize slow queries
- Consider materialized views for complex aggregations

## Integration with Rust Daemon

### Scheduling

Python scripts can be:

1. **Manually run** - For development and one-off analysis
2. **Cron scheduled** - For periodic reports and batch analysis
3. **Triggered by Rust** - Daemon could execute Python scripts after data ingestion

Example crontab:

```cron
# Run daily flood analysis at 6 AM
0 6 * * * cd /home/user/flopro/python_analysis && ./venv/bin/python scripts/analyze_events.py

# Generate weekly report every Monday at 8 AM
0 8 * * 1 cd /home/user/flopro/python_analysis && ./venv/bin/python scripts/weekly_report.py
```

### Future: REST API

When the Rust daemon implements an HTTP API:

```python
import requests

# Fetch current conditions (hypothetical)
response = requests.get('http://localhost:8080/api/stations/05568500/current')
data = response.json()

if data['stage_ft'] > data['flood_stage_ft']:
    # Trigger analysis
    run_flood_analysis(data['site_code'])
```

## Migration Notes

The following functionality has been moved from Rust to Python:

- **Precursor window detection** - Was in `src/analysis/flood_events.rs`
- **Rise rate calculations** - Statistical metrics
- **Multi-source correlation** - USGS + CWMS linkage
- **Event classification** - Severity scoring
- **Complex backwater analysis** - Beyond simple differential

The Rust daemon now focuses on:
- Data ingestion and validation
- Simple threshold monitoring
- Staleness tracking
- Database integrity

This separation allows rapid iteration on analysis algorithms while maintaining a stable, reliable data platform.
