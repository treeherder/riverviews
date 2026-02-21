"""Segmented linear regression for flood analysis.

Implements piecewise linear models to capture non-linear relationships
in hydrological data, such as:
- Stage-discharge curves with floodplain overflow
- Upstream-downstream relationships with regime changes
- Backwater effects with threshold transitions
"""

import numpy as np
import pandas as pd
from typing import Optional, Tuple, List
from dataclasses import dataclass
import logging

try:
    import pwlf  # Piecewise Linear Fit
except ImportError:
    pwlf = None

logger = logging.getLogger(__name__)


@dataclass
class RegressionResult:
    """Results from segmented linear regression."""
    breakpoints: np.ndarray
    slopes: np.ndarray
    intercepts: np.ndarray
    r_squared: float
    rmse: float
    n_segments: int
    
    def predict(self, x: np.ndarray) -> np.ndarray:
        """Predict y values using the fitted model.
        
        Args:
            x: Input values
            
        Returns:
            Predicted values
        """
        x = np.asarray(x)
        y_pred = np.zeros_like(x, dtype=float)
        
        for i in range(self.n_segments):
            if i == 0:
                mask = x <= self.breakpoints[i + 1]
            elif i == self.n_segments - 1:
                mask = x > self.breakpoints[i]
            else:
                mask = (x > self.breakpoints[i]) & (x <= self.breakpoints[i + 1])
            
            y_pred[mask] = self.slopes[i] * x[mask] + self.intercepts[i]
        
        return y_pred
    
    def __str__(self) -> str:
        """Human-readable summary of regression."""
        lines = [
            f"Segmented Linear Regression ({self.n_segments} segments)",
            f"  R² = {self.r_squared:.4f}",
            f"  RMSE = {self.rmse:.4f}",
            f"  Breakpoints: {self.breakpoints}",
        ]
        return "\n".join(lines)


def fit_segmented_regression(
    x: np.ndarray,
    y: np.ndarray,
    n_segments: int = 2,
    breakpoint_init: Optional[np.ndarray] = None
) -> RegressionResult:
    """Fit a segmented (piecewise) linear regression model.
    
    Args:
        x: Independent variable (e.g., discharge)
        y: Dependent variable (e.g., stage)
        n_segments: Number of linear segments
        breakpoint_init: Initial guess for breakpoint locations (optional)
        
    Returns:
        RegressionResult with fitted parameters
        
    Raises:
        ImportError: If pwlf package not installed
        ValueError: If data is invalid
    """
    if pwlf is None:
        raise ImportError(
            "pwlf package required for segmented regression.\n"
            "Install with: pip install pwlf"
        )
    
    x = np.asarray(x).flatten()
    y = np.asarray(y).flatten()
    
    if len(x) != len(y):
        raise ValueError(f"x and y must have same length (got {len(x)} and {len(y)})")
    
    if len(x) < n_segments + 1:
        raise ValueError(f"Need at least {n_segments + 1} data points for {n_segments} segments")
    
    # Remove NaN values
    mask = ~(np.isnan(x) | np.isnan(y))
    x = x[mask]
    y = y[mask]
    
    logger.info(f"Fitting {n_segments}-segment regression to {len(x)} data points")
    
    # Create piecewise linear fit object
    model = pwlf.PiecewiseLinFit(x, y)
    
    # Fit the model
    if breakpoint_init is not None:
        model.fit_with_breaks(breakpoint_init)
    else:
        # Automatically determine breakpoints
        model.fit(n_segments)
    
    # Get breakpoints and slopes
    breakpoints = model.fit_breaks
    slopes = model.slopes
    intercepts = model.intercepts
    
    # Calculate R² and RMSE
    y_pred = model.predict(x)
    ss_res = np.sum((y - y_pred) ** 2)
    ss_tot = np.sum((y - np.mean(y)) ** 2)
    r_squared = 1 - (ss_res / ss_tot)
    rmse = np.sqrt(np.mean((y - y_pred) ** 2))
    
    logger.info(f"Fit complete: R² = {r_squared:.4f}, RMSE = {rmse:.4f}")
    
    return RegressionResult(
        breakpoints=breakpoints,
        slopes=slopes,
        intercepts=intercepts,
        r_squared=r_squared,
        rmse=rmse,
        n_segments=n_segments
    )


def fit_stage_discharge(
    discharge_cfs: pd.Series,
    stage_ft: pd.Series,
    n_segments: int = 3
) -> RegressionResult:
    """Fit segmented regression to stage-discharge relationship.
    
    Common breakpoints in stage-discharge curves:
    - Segment 1: In-channel flow
    - Segment 2: Bank-full to floodplain
    - Segment 3: Widespread flooding
    
    Args:
        discharge_cfs: Streamflow in cubic feet per second
        stage_ft: River stage in feet
        n_segments: Number of segments (default 3)
        
    Returns:
        RegressionResult
    """
    logger.info("Fitting stage-discharge relationship")
    
    # Stage-discharge: discharge is independent variable (x), stage is dependent (y)
    return fit_segmented_regression(
        x=discharge_cfs.values,
        y=stage_ft.values,
        n_segments=n_segments
    )


def find_optimal_segments(
    x: np.ndarray,
    y: np.ndarray,
    max_segments: int = 5
) -> Tuple[int, List[float]]:
    """Find optimal number of segments using elbow method.
    
    Fits models with 1 to max_segments and identifies where
    improvement in R² starts to diminish.
    
    Args:
        x: Independent variable
        y: Dependent variable
        max_segments: Maximum number of segments to try
        
    Returns:
        Tuple of (optimal_n_segments, r_squared_values)
    """
    if pwlf is None:
        raise ImportError("pwlf package required")
    
    r_squared_values = []
    
    for n in range(1, max_segments + 1):
        result = fit_segmented_regression(x, y, n_segments=n)
        r_squared_values.append(result.r_squared)
        logger.info(f"  {n} segments: R² = {result.r_squared:.4f}")
    
    # Simple elbow detection: find where improvement < 0.01
    improvements = np.diff(r_squared_values)
    optimal = np.argmax(improvements < 0.01) + 1
    
    if optimal == 0:  # All improvements significant
        optimal = max_segments
    
    logger.info(f"Optimal number of segments: {optimal}")
    return optimal, r_squared_values


if __name__ == "__main__":
    # Example usage
    logging.basicConfig(level=logging.INFO)
    
    # Generate synthetic stage-discharge data with breakpoint
    np.random.seed(42)
    discharge = np.linspace(1000, 50000, 200)
    
    # Two-regime relationship (channel vs floodplain)
    stage = np.where(
        discharge < 20000,
        10 + discharge / 2000,  # In-channel: gentle slope
        20 + (discharge - 20000) / 10000  # Floodplain: steep slope
    )
    stage += np.random.normal(0, 0.5, len(stage))  # Add noise
    
    # Fit segmented regression
    result = fit_segmented_regression(discharge, stage, n_segments=2)
    print(result)
    
    # Predict stage at 25,000 cfs
    predicted_stage = result.predict([25000])
    print(f"\nPredicted stage at 25,000 cfs: {predicted_stage[0]:.2f} ft")
