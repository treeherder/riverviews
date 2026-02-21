# FloML - Flood Machine Learning

Statistical analysis and machine learning for flood prediction using data curated by the FloPro monitoring daemon.

## Overview

FloML performs complex analysis on flood monitoring data:
- **Multivariate segmented linear regression** - Model non-linear stage-discharge relationships
- **Multi-station correlation** - Upstream-downstream timing and magnitudes
- **Precursor pattern detection** - Identify pre-flood indicators
- **Backwater influence modeling** - Quantify Mississippi River effects on Illinois River

The Rust daemon (`flomon_service`) curates reliable data; FloML performs the statistical heavy lifting.

## Setup

```bash
# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt

# Configure database connection
cp .env.example .env
# Edit .env with your database credentials
```

## Project Structure

```
floml/
├── floml/                  # Main Python package
│   ├── __init__.py
│   ├── db.py              # Database connection utilities
│   ├── regression.py      # Segmented linear regression models
│   ├── correlation.py     # Multi-station analysis
│   └── precursors.py      # Flood precursor detection
├── scripts/               # Standalone analysis scripts
│   └── analyze_events.py  # Example analysis workflow
├── notebooks/             # Jupyter notebooks for exploration
├── tests/                 # Unit tests
├── requirements.txt
└── README.md
```

## Quick Start

```python
from floml.db import get_engine
from floml.regression import fit_stage_discharge
import pandas as pd

# Connect to database
engine = get_engine()

# Load data for a station
data = pd.read_sql("""
    SELECT reading_time, 
           MAX(CASE WHEN parameter_code = '00065' THEN value END) as stage_ft,
           MAX(CASE WHEN parameter_code = '00060' THEN value END) as discharge_cfs
    FROM usgs_raw.gauge_readings
    WHERE site_code = '05568500'
    GROUP BY reading_time
    HAVING MAX(CASE WHEN parameter_code = '00065' THEN value END) IS NOT NULL
       AND MAX(CASE WHEN parameter_code = '00060' THEN value END) IS NOT NULL
    ORDER BY reading_time
""", engine)

# Fit segmented regression
model = fit_stage_discharge(data['discharge_cfs'], data['stage_ft'], n_segments=3)
print(f"Breakpoints: {model.fit_breaks}")
```

## Segmented Linear Regression

For stage-discharge relationships that aren't linear (common in rivers with:
- Channel overflow into floodplain
- Backwater effects
- Ice jams

We use piecewise linear fitting to capture regime changes:

```python
from floml.regression import SegmentedRegressionModel

# Automatically find optimal breakpoints
model = SegmentedRegressionModel(n_segments=3)
model.fit(discharge, stage)

# Predict stage from discharge
predicted_stage = model.predict(new_discharge_values)

# Get breakpoint locations
breakpoints = model.breakpoints
```

## Data Access Patterns

All analysis reads from the PostgreSQL database curated by the Rust daemon:

- **`usgs_raw.gauge_readings`** - Time series stage and discharge data
- **`nws.flood_events`** - Historical flood events for training
- **`usace.cwms_timeseries`** - Mississippi River data for backwater analysis

Results are written back to:

- **`flood_analysis.*`** - Analysis outputs, predictions, model parameters

## Example Scripts

```bash
# Run flood event analysis
python scripts/analyze_events.py --site-code 05568500

# Interactive exploration
jupyter notebook notebooks/
```

## Development

```bash
# Run tests
pytest tests/

# Add new analysis module
# 1. Create floml/new_module.py
# 2. Add tests in tests/test_new_module.py
# 3. Import in floml/__init__.py
```

## References

- Rust daemon documentation: `../flomon_service/docs/PYTHON_INTEGRATION.md`
- Database schema: `../flomon_service/sql/`
- Segmented regression: Muggeo, V. M. (2003). "Estimating regression models with unknown break-points."
