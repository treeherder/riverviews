/// Development mode utilities for working with historical data
/// 
/// When live USGS data is unavailable, use this module to replay
/// historical data for testing and development.

use postgres::Client;
use chrono::{DateTime, Utc, Duration};
use crate::model::GaugeReading;

/// Configuration for development mode data replay
pub struct DevMode {
    /// Simulate data as if it's this many days in the past
    pub days_offset: i64,
    /// Update interval in seconds (default: 900 = 15 minutes)
    pub update_interval_secs: i64,
}

impl DevMode {
    /// Create a new dev mode configuration
    /// 
    /// # Arguments
    /// * `days_offset` - Replay data from this many days ago
    pub fn new(days_offset: i64) -> Self {
        Self {
            days_offset,
            update_interval_secs: 900, // 15 minutes
        }
    }
    
    /// Fetch historical readings as if they were current
    /// 
    /// Returns readings from `days_offset` days ago, simulating live data
    pub fn fetch_simulated_current_readings(
        &self,
        client: &mut Client,
        site_codes: &[String],
    ) -> Result<Vec<GaugeReading>, postgres::Error> {
        
        let simulated_now = Utc::now() - Duration::days(self.days_offset);
        let window_start = simulated_now - Duration::seconds(self.update_interval_secs * 2);
        
        let query = "
            SELECT DISTINCT ON (site_code, parameter_code)
                site_code,
                s.site_name,
                measurement_time,
                parameter_code,
                value,
                unit
            FROM usgs_raw.gauge_readings g
            JOIN usgs_raw.sites s ON g.site_code = s.site_code
            WHERE g.site_code = ANY($1)
              AND measurement_time >= $2
              AND measurement_time <= $3
            ORDER BY g.site_code, parameter_code, measurement_time DESC
        ";
        
        let rows = client.query(
            query,
            &[&site_codes, &window_start, &simulated_now],
        )?;
        
        let mut readings = Vec::new();
        for row in rows {
            readings.push(GaugeReading {
                site_code: row.get(0),
                site_name: row.get(1),
                datetime: row.get::<_, DateTime<Utc>>(2).to_rfc3339(),
                parameter_code: row.get(3),
                value: row.get(4),
                unit: row.get(5),
                qualifier: String::new(),
            });
        }
        
        Ok(readings)
    }
    
    /// Get available data date range for a site
    pub fn get_data_range(
        client: &mut Client,
        site_code: &str,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>, postgres::Error> {
        
        let row = client.query_one(
            "SELECT MIN(measurement_time), MAX(measurement_time)
             FROM usgs_raw.gauge_readings
             WHERE site_code = $1",
            &[&site_code],
        )?;
        
        let min: Option<DateTime<Utc>> = row.get(0);
        let max: Option<DateTime<Utc>> = row.get(1);
        
        match (min, max) {
            (Some(start), Some(end)) => Ok(Some((start, end))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dev_mode_creation() {
        let dev = DevMode::new(365);
        assert_eq!(dev.days_offset, 365);
        assert_eq!(dev.update_interval_secs, 900);
    }
}
