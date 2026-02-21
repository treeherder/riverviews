"""Multi-station correlation analysis.

Analyzes relationships between upstream and downstream gauges,
including timing lags, magnitude correlations, and flood wave propagation.
"""

import numpy as np
import pandas as pd
from typing import Tuple, Optional, Dict
from dataclasses import dataclass
from scipy import stats
from scipy.signal import correlate
import logging

logger = logging.getLogger(__name__)


@dataclass
class CorrelationResult:
    """Results from correlation analysis between two stations."""
    pearson_r: float
    p_value: float
    lag_hours: int
    r_squared: float
    slope: float
    intercept: float
    sample_size: int
    
    def __str__(self) -> str:
        return (
            f"Correlation: r = {self.pearson_r:.3f} (p = {self.p_value:.4f})\n"
            f"  Lag: {self.lag_hours} hours\n"
            f"  R² = {self.r_squared:.3f}\n"
            f"  Linear fit: y = {self.slope:.3f}x + {self.intercept:.2f}\n"
            f"  Sample size: {self.sample_size}"
        )


def find_optimal_lag(
    upstream: pd.Series,
    downstream: pd.Series,
    max_lag_hours: int = 48
) -> int:
    """Find optimal time lag between upstream and downstream stations.
    
    Uses cross-correlation to find the lag that maximizes correlation.
    
    Args:
        upstream: Upstream station time series (indexed by timestamp)
        downstream: Downstream station time series (indexed by timestamp)
        max_lag_hours: Maximum lag to consider
        
    Returns:
        Optimal lag in hours (positive = downstream lags behind upstream)
    """
    # Resample to hourly if not already
    upstream_hourly = upstream.resample('1H').mean()
    downstream_hourly = downstream.resample('1H').mean()
    
    # Align and drop NaN
    df = pd.DataFrame({
        'upstream': upstream_hourly,
        'downstream': downstream_hourly
    }).dropna()
    
    if len(df) < max_lag_hours:
        logger.warning(f"Only {len(df)} overlapping points, lag detection may be unreliable")
    
    # Compute cross-correlation
    correlation = correlate(
        df['downstream'].values - df['downstream'].mean(),
        df['upstream'].values - df['upstream'].mean(),
        mode='same'
    )
    
    # Find lag with maximum correlation
    center = len(correlation) // 2
    search_range = min(max_lag_hours, center)
    
    lag_index = np.argmax(correlation[center-search_range:center+search_range]) - search_range
    
    logger.info(f"Optimal lag: {lag_index} hours")
    return lag_index


def correlate_stations(
    upstream: pd.Series,
    downstream: pd.Series,
    lag_hours: Optional[int] = None,
    auto_detect_lag: bool = True
) -> CorrelationResult:
    """Correlate upstream and downstream station data.
    
    Args:
        upstream: Upstream station values (indexed by timestamp)
        downstream: Downstream station values (indexed by timestamp)
        lag_hours: Manual lag in hours (if None and auto_detect_lag=True, auto-detect)
        auto_detect_lag: If True, automatically detect optimal lag
        
    Returns:
        CorrelationResult
    """
    if auto_detect_lag and lag_hours is None:
        lag_hours = find_optimal_lag(upstream, downstream)
    elif lag_hours is None:
        lag_hours = 0
    
    # Shift downstream by lag
    downstream_shifted = downstream.shift(freq=f'{-lag_hours}H')
    
    # Align data
    df = pd.DataFrame({
        'upstream': upstream,
        'downstream': downstream_shifted
    }).dropna()
    
    if len(df) < 10:
        raise ValueError(f"Insufficient overlapping data points: {len(df)}")
    
    # Compute Pearson correlation
    pearson_r, p_value = stats.pearsonr(df['upstream'], df['downstream'])
    
    # Linear regression
    slope, intercept, r_value, _, _ = stats.linregress(df['upstream'], df['downstream'])
    r_squared = r_value ** 2
    
    logger.info(f"Correlation: r = {pearson_r:.3f}, lag = {lag_hours}h, n = {len(df)}")
    
    return CorrelationResult(
        pearson_r=pearson_r,
        p_value=p_value,
        lag_hours=lag_hours,
        r_squared=r_squared,
        slope=slope,
        intercept=intercept,
        sample_size=len(df)
    )


def predict_downstream(
    upstream_value: float,
    correlation: CorrelationResult
) -> Tuple[float, int]:
    """Predict downstream station value from upstream value.
    
    Args:
        upstream_value: Current upstream stage or discharge
        correlation: Previously computed correlation result
        
    Returns:
        Tuple of (predicted_downstream_value, lag_hours)
    """
    predicted = correlation.slope * upstream_value + correlation.intercept
    return predicted, correlation.lag_hours


def analyze_station_network(
    data: Dict[str, pd.Series],
    station_order: list
) -> pd.DataFrame:
    """Analyze correlations across a network of stations.
    
    Args:
        data: Dictionary mapping station codes to time series
        station_order: List of station codes in upstream-to-downstream order
        
    Returns:
        DataFrame with pairwise correlation statistics
    """
    results = []
    
    for i in range(len(station_order) - 1):
        upstream_code = station_order[i]
        downstream_code = station_order[i + 1]
        
        logger.info(f"Analyzing {upstream_code} → {downstream_code}")
        
        try:
            corr = correlate_stations(
                data[upstream_code],
                data[downstream_code]
            )
            
            results.append({
                'upstream': upstream_code,
                'downstream': downstream_code,
                'correlation': corr.pearson_r,
                'p_value': corr.p_value,
                'lag_hours': corr.lag_hours,
                'r_squared': corr.r_squared,
                'slope': corr.slope,
                'sample_size': corr.sample_size
            })
        except Exception as e:
            logger.error(f"Error analyzing {upstream_code} → {downstream_code}: {e}")
    
    return pd.DataFrame(results)


if __name__ == "__main__":
    # Example usage
    logging.basicConfig(level=logging.INFO)
    
    # Generate synthetic upstream/downstream data with 12-hour lag
    dates = pd.date_range('2024-01-01', periods=1000, freq='1H')
    
    # Upstream station: sine wave + noise
    upstream = pd.Series(
        10 + 5 * np.sin(np.arange(1000) * 2 * np.pi / 168) + np.random.normal(0, 0.5, 1000),
        index=dates
    )
    
    # Downstream: lagged + attenuated + noise
    downstream = pd.Series(
        9 + 4 * np.sin((np.arange(1000) - 12) * 2 * np.pi / 168) + np.random.normal(0, 0.5, 1000),
        index=dates
    )
    
    # Analyze correlation
    result = correlate_stations(upstream, downstream)
    print(result)
    
    # Predict downstream from upstream value
    upstream_current = 13.5
    predicted, lag = predict_downstream(upstream_current, result)
    print(f"\nIf upstream is {upstream_current:.1f} ft now,")
    print(f"downstream will be {predicted:.1f} ft in {lag} hours")
