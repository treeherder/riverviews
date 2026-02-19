/// Real-time monitoring service with database-backed staleness tracking.
///
/// ## Architecture: Hybrid Database + In-Memory
///
/// **Database (source of truth):**
/// - `monitoring_state` table tracks polling state per station
/// - `station_health` view provides current health dashboard
/// - Survives service restarts
/// - Queryable for historical analysis
///
/// **In-Memory (performance):**
/// - Cache of latest readings to avoid DB queries on every check
/// - Quick staleness checks without hitting database
/// - Refreshed from DB on service startup
/// - Updated on each successful poll
///
/// **Flow:**
/// 1. Service polls USGS API every 15 minutes
/// 2. Store new readings in `gauge_readings` table
/// 3. Update `monitoring_state` via `update_monitoring_state()` function
/// 4. Update in-memory cache from DB
/// 5. Staleness checks use cache (fall back to DB if needed)
///
/// This approach provides:
/// - Persistence (database survives crashes)
/// - Performance (in-memory for hot path)
/// - Auditability (DB tracks state changes over time)
/// - Simplicity (no dual state files to keep in sync)

use crate::model::GaugeReading;
use chrono::{DateTime, Utc};
use postgres::Client;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// In-Memory State Cache
// ---------------------------------------------------------------------------

/// Cached state for a single station's parameter.
/// Refreshed from database periodically.
#[derive(Debug, Clone)]
pub struct StationCache {
    pub site_code: String,
    pub parameter_code: String,
    pub latest_reading_time: Option<DateTime<Utc>>,
    pub latest_reading_value: Option<f64>,
    pub staleness_threshold_minutes: i32,
    pub status: StationStatus,
    pub last_poll_attempted: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StationStatus {
    Active,
    Degraded,  // Stale data but station exists
    Offline,   // No data available
    Unknown,
}

impl StationStatus {
    fn from_str(s: &str) -> Self {
        match s {
            "active" => StationStatus::Active,
            "degraded" => StationStatus::Degraded,
            "offline" => StationStatus::Offline,
            _ => StationStatus::Unknown,
        }
    }
}

/// In-memory cache of station states.
/// Key: (site_code, parameter_code)
pub struct MonitoringCache {
    cache: HashMap<(String, String), StationCache>,
    last_refresh: DateTime<Utc>,
}

impl MonitoringCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            last_refresh: Utc::now(),
        }
    }

    /// Load cache from database on startup or refresh.
    pub fn refresh_from_db(&mut self, client: &mut Client) -> Result<(), Box<dyn std::error::Error>> {
        let rows = client.query(
            "SELECT site_code, parameter_code, latest_reading_time, \
                    latest_reading_value, staleness_threshold_minutes, \
                    status, last_poll_attempted \
             FROM usgs_raw.monitoring_state",
            &[],
        )?;

        self.cache.clear();

        for row in rows {
            let site_code: String = row.get(0);
            let parameter_code: String = row.get(1);
            let latest_reading_time: Option<DateTime<Utc>> = row.get(2);
            let latest_reading_value: Option<f64> = row.get::<_, Option<f64>>(3);
            let staleness_threshold_minutes: i32 = row.get(4);
            let status_str: String = row.get(5);
            let last_poll_attempted: Option<DateTime<Utc>> = row.get(6);

            let cache_entry = StationCache {
                site_code: site_code.clone(),
                parameter_code: parameter_code.clone(),
                latest_reading_time,
                latest_reading_value,
                staleness_threshold_minutes,
                status: StationStatus::from_str(&status_str),
                last_poll_attempted,
            };

            self.cache.insert((site_code, parameter_code), cache_entry);
        }

        self.last_refresh = Utc::now();
        Ok(())
    }

    /// Get cached station state (fast path).
    pub fn get(&self, site_code: &str, parameter_code: &str) -> Option<&StationCache> {
        self.cache.get(&(site_code.to_string(), parameter_code.to_string()))
    }

    /// Check if data is stale using cached threshold.
    pub fn is_stale(&self, site_code: &str, parameter_code: &str, now: DateTime<Utc>) -> bool {
        if let Some(cached) = self.get(site_code, parameter_code) {
            if let Some(reading_time) = cached.latest_reading_time {
                let age_minutes = (now - reading_time).num_minutes();
                return age_minutes > cached.staleness_threshold_minutes as i64;
            }
        }
        true // Unknown stations are stale by default
    }

    /// Get all offline or degraded stations.
    pub fn unhealthy_stations(&self) -> Vec<&StationCache> {
        self.cache
            .values()
            .filter(|s| s.status == StationStatus::Offline || s.status == StationStatus::Degraded)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Database Operations
// ---------------------------------------------------------------------------

/// Record a polling attempt in the database and update state.
pub fn record_poll_result(
    client: &mut Client,
    site_code: &str,
    parameter_code: &str,
    success: bool,
    readings: &[GaugeReading],
) -> Result<(), Box<dyn std::error::Error>> {
    // Find latest reading for this site/parameter
    let latest = readings
        .iter()
        .filter(|r| r.site_code == site_code && r.parameter_code == parameter_code)
        .max_by_key(|r| &r.datetime);

    let (latest_time, latest_value) = if let Some(reading) = latest {
        // Parse datetime from string
        let dt = chrono::DateTime::parse_from_rfc3339(&reading.datetime)
            .map(|dt| dt.with_timezone(&Utc))
            .ok();
        (dt, Some(reading.value))
    } else {
        (None, None)
    };

    // Call database function to update state
    client.execute(
        "SELECT usgs_raw.update_monitoring_state($1, $2, $3, $4, $5, $6)",
        &[
            &site_code,
            &parameter_code,
            &success,
            &(readings.len() as i32),
            &latest_time,
            &latest_value,
        ],
    )?;

    Ok(())
}

/// Get current health status from database (bypass cache).
pub fn get_station_health(
    client: &mut Client,
) -> Result<Vec<StationHealthRow>, Box<dyn std::error::Error>> {
    let rows = client.query("SELECT * FROM usgs_raw.station_health", &[])?;

    let mut results = Vec::new();
    for row in rows {
        results.push(StationHealthRow {
            site_code: row.get(0),
            site_name: row.get(1),
            parameter_code: row.get(2),
            status: row.get(3),
            status_since: row.get(4),
            is_stale: row.get(5),
            stale_since: row.get(6),
            latest_reading_time: row.get(7),
            latest_reading_value: row.get::<_, Option<f64>>(8),
            age_minutes: row.get::<_, Option<f64>>(9),
            staleness_threshold_minutes: row.get(10),
            last_poll_attempted: row.get(11),
            last_poll_succeeded: row.get(12),
            consecutive_failures: row.get(13),
        });
    }

    Ok(results)
}

#[derive(Debug)]
pub struct StationHealthRow {
    pub site_code: String,
    pub site_name: String,
    pub parameter_code: String,
    pub status: String,
    pub status_since: Option<DateTime<Utc>>,
    pub is_stale: Option<bool>,
    pub stale_since: Option<DateTime<Utc>>,
    pub latest_reading_time: Option<DateTime<Utc>>,
    pub latest_reading_value: Option<f64>,
    pub age_minutes: Option<f64>,
    pub staleness_threshold_minutes: i32,
    pub last_poll_attempted: Option<DateTime<Utc>>,
    pub last_poll_succeeded: Option<DateTime<Utc>>,
    pub consecutive_failures: i32,
}

// ---------------------------------------------------------------------------
// Example Real-Time Service Loop
// ---------------------------------------------------------------------------

/// Example main monitoring loop demonstrating hybrid approach.
/// 
/// This would run continuously in main.rs:
/// ```no_run
/// loop {
///     // 1. Poll USGS API
///     let readings = fetch_latest_from_usgs()?;
///     
///     // 2. Store in database
///     store_readings(&mut db, &readings)?;
///     
///     // 3. Update monitoring state
///     for station in stations {
///         record_poll_result(&mut db, &station.site_code, "00060", true, &readings)?;
///     }
///     
///     // 4. Refresh in-memory cache
///     cache.refresh_from_db(&mut db)?;
///     
///     // 5. Check for alerts
///     let unhealthy = cache.unhealthy_stations();
///     if !unhealthy.is_empty() {
///         send_alerts(&unhealthy)?;
///     }
///     
///     // 6. Sleep until next poll
///     sleep(Duration::from_secs(15 * 60));
/// }
/// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_staleness_check() {
        let mut cache = MonitoringCache::new();
        
        // Simulate cached station with 60-minute threshold
        let station = StationCache {
            site_code: "05568500".to_string(),
            parameter_code: "00060".to_string(),
            latest_reading_time: Some(Utc::now() - chrono::Duration::minutes(90)),
            latest_reading_value: Some(42000.0),
            staleness_threshold_minutes: 60,
            status: StationStatus::Active,
            last_poll_attempted: Some(Utc::now()),
        };
        
        cache.cache.insert(
            ("05568500".to_string(), "00060".to_string()),
            station,
        );

        // Should be stale (90 min > 60 min threshold)
        assert!(cache.is_stale("05568500", "00060", Utc::now()));
    }

    #[test]
    fn test_cache_fresh_data() {
        let mut cache = MonitoringCache::new();
        
        let station = StationCache {
            site_code: "05568500".to_string(),
            parameter_code: "00060".to_string(),
            latest_reading_time: Some(Utc::now() - chrono::Duration::minutes(10)),
            latest_reading_value: Some(42000.0),
            staleness_threshold_minutes: 60,
            status: StationStatus::Active,
            last_poll_attempted: Some(Utc::now()),
        };
        
        cache.cache.insert(
            ("05568500".to_string(), "00060".to_string()),
            station,
        );

        // Should NOT be stale (10 min < 60 min threshold)
        assert!(!cache.is_stale("05568500", "00060", Utc::now()));
    }
}
