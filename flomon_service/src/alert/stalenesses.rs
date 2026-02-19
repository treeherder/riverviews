///is_stale + staleness tests
/// will depend on chrono and may eventually require clock injection for testing. 
/// Separating these functions out into their own module keeps the core data model and ingestion logic free of external dependencies and makes it easier to test the staleness logic in isolation.
/// /// Gauge reading staleness detection.
///
/// USGS gauges update every 15 minutes under normal conditions. During active
/// flood events, stale data is dangerous — a sensor outage or communication
/// failure may not be immediately obvious from the dashboard. This module
/// provides staleness checking so the alerting system can flag gaps.
///
/// # Clock injection
/// All functions accept a `now: DateTime<Utc>` parameter rather than calling
/// `Utc::now()` internally. This makes staleness purely deterministic in
/// tests without mocking or time manipulation.

use crate::model::GaugeReading;

// ---------------------------------------------------------------------------
// Staleness check
// ---------------------------------------------------------------------------

/// Returns `true` if the reading's datetime is older than `max_age_minutes`
/// relative to `now`.
///
/// Staleness is defined as strictly greater than the threshold:
///   age > max_age_minutes  →  stale
///   age == max_age_minutes →  not stale
///
/// Returns an error if the reading's datetime string cannot be parsed.
/// Callers should treat parse failures as stale (fail-safe default).
///
/// # Typical thresholds
/// - Normal monitoring: 60 minutes (four missed 15-min updates)
/// - Active flood event: 20 minutes (one missed update)
pub fn is_stale_at(
    reading: &GaugeReading,
    max_age_minutes: u64,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<bool, String> {
    // TODO: implement with chrono.
    //   1. Parse reading.datetime as DateTime<FixedOffset>.
    //   2. Convert to Utc.
    //   3. Compute (now - reading_time).num_minutes() as u64.
    //   4. Return Ok(age_minutes > max_age_minutes).
    //   5. Return Err(...) if datetime parsing fails.
    let _ = (reading, max_age_minutes, now);
    unimplemented!("is_stale_at: parse datetime and compare against now")
}

/// Convenience wrapper that uses the real current time.
/// Use `is_stale_at` in tests to keep them deterministic.
pub fn is_stale(reading: &GaugeReading, max_age_minutes: u64) -> Result<bool, String> {
    is_stale_at(reading, max_age_minutes, chrono::Utc::now())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GaugeReading;
    use chrono::{TimeZone, Utc};

    fn reading_at(datetime: &str) -> GaugeReading {
        GaugeReading {
            site_code: "05568500".to_string(),
            site_name: "Illinois River at Kingston Mines, IL".to_string(),
            parameter_code: "00060".to_string(),
            unit: "ft3/s".to_string(),
            value: 42_300.0,
            datetime: datetime.to_string(),
            qualifier: "P".to_string(),
        }
    }

    /// A fixed "now" used across all tests: 2024-05-01 13:00:00 UTC.
    fn fixed_now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 5, 1, 13, 0, 0).unwrap()
    }

    // --- Not stale ----------------------------------------------------------

    #[test]
    fn test_reading_5_minutes_old_is_not_stale() {
        // Reading at 12:55 UTC, now is 13:00 UTC — age is 5 minutes.
        let reading = reading_at("2024-05-01T12:55:00.000+00:00");
        let stale = is_stale_at(&reading, 15, fixed_now())
            .expect("valid datetime should not error");
        assert!(!stale, "5-minute-old reading should not be stale with 15-min threshold");
    }

    #[test]
    fn test_reading_exactly_at_threshold_is_not_stale() {
        // Age == threshold should NOT be considered stale (strictly greater than).
        let reading = reading_at("2024-05-01T12:45:00.000+00:00"); // 15 min ago
        let stale = is_stale_at(&reading, 15, fixed_now())
            .expect("valid datetime should not error");
        assert!(
            !stale,
            "reading exactly at threshold (15 min) should not be stale — \
             staleness is strictly greater than, not >=",
        );
    }

    #[test]
    fn test_reading_with_central_time_offset_parsed_correctly() {
        // USGS returns Central time with -05:00 or -06:00 offset.
        // 2024-05-01T08:00:00-05:00 == 2024-05-01T13:00:00Z — exactly 0 min old.
        let reading = reading_at("2024-05-01T08:00:00.000-05:00");
        let stale = is_stale_at(&reading, 15, fixed_now())
            .expect("timezone-offset datetime should parse correctly");
        assert!(!stale, "reading from 0 minutes ago should not be stale");
    }

    // --- Stale --------------------------------------------------------------

    #[test]
    fn test_reading_one_minute_past_threshold_is_stale() {
        // Age is 16 minutes, threshold is 15 — should be stale.
        let reading = reading_at("2024-05-01T12:44:00.000+00:00");
        let stale = is_stale_at(&reading, 15, fixed_now())
            .expect("valid datetime should not error");
        assert!(stale, "16-minute-old reading should be stale with 15-min threshold");
    }

    #[test]
    fn test_reading_from_hours_ago_is_stale() {
        let reading = reading_at("2024-05-01T09:00:00.000+00:00"); // 4 hours ago
        let stale = is_stale_at(&reading, 60, fixed_now())
            .expect("valid datetime should not error");
        assert!(stale, "4-hour-old reading should be stale with 60-min threshold");
    }

    #[test]
    fn test_reading_from_2020_is_stale_under_any_threshold() {
        let reading = reading_at("2020-01-01T00:00:00.000+00:00");
        let stale = is_stale_at(&reading, 60, fixed_now())
            .expect("old but valid datetime should parse");
        assert!(stale, "reading from 2020 should be stale under any reasonable threshold");
    }

    // --- Error handling -----------------------------------------------------

    #[test]
    fn test_invalid_datetime_returns_error() {
        let reading = reading_at("not-a-datetime");
        let result = is_stale_at(&reading, 15, fixed_now());
        assert!(
            result.is_err(),
            "unparseable datetime should return Err, got {:?}",
            result
        );
    }

    #[test]
    fn test_empty_datetime_returns_error() {
        let reading = reading_at("");
        let result = is_stale_at(&reading, 15, fixed_now());
        assert!(result.is_err(), "empty datetime should return Err");
    }

    // --- Threshold variation ------------------------------------------------

    #[test]
    fn test_same_reading_stale_under_tight_threshold_not_under_loose() {
        // Reading is 30 minutes old.
        let reading = reading_at("2024-05-01T12:30:00.000+00:00");
        let stale_20 = is_stale_at(&reading, 20, fixed_now()).expect("should not error");
        let stale_60 = is_stale_at(&reading, 60, fixed_now()).expect("should not error");
        assert!(stale_20, "30-min-old reading is stale under a 20-min threshold");
        assert!(!stale_60, "30-min-old reading is not stale under a 60-min threshold");
    }
}
