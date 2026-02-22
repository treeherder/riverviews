/// Integration tests for daemon lifecycle behavior
///
/// These tests define and verify the complete daemon startup and operation:
/// 1. Database existence and schema validation
/// 2. Data staleness detection
/// 3. Backfill missing data to current
/// 4. Continuous monitoring and warehousing
///
/// These tests serve as a specification for implementation and can be
/// run incrementally as the daemon matures.
///
/// Prerequisites:
/// - PostgreSQL running with flopro_db database
/// - DATABASE_URL set in .env
/// - All SQL migrations applied
///
/// Run with: cargo test --test daemon_lifecycle -- --test-threads=1

use flomon_service::db;
use flomon_service::stations;
use postgres::{Client, NoTls};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use std::env;

// ---------------------------------------------------------------------------
// Test Helpers
// ---------------------------------------------------------------------------

fn setup_test_db() -> Client {
    dotenv::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    Client::connect(&database_url, NoTls).expect("Failed to connect to test database")
}

fn cleanup_test_data(client: &mut Client) {
    // Clean up test data between tests
    let _ = client.execute("DELETE FROM usgs_raw.gauge_readings WHERE site_code LIKE 'TEST%'", &[]);
    let _ = client.execute("DELETE FROM usgs_raw.monitoring_state WHERE site_code LIKE 'TEST%'", &[]);
}

// ---------------------------------------------------------------------------
// 1. Database Existence and Schema Validation
// ---------------------------------------------------------------------------

#[test]
fn test_daemon_validates_database_schemas_on_startup() {
    // The daemon should verify all required schemas exist before proceeding
    let result = db::connect_and_verify(&["usgs_raw", "nws", "usace"]);
    
    assert!(
        result.is_ok(),
        "Daemon should successfully verify all required database schemas exist"
    );
}

#[test]
fn test_daemon_fails_gracefully_when_schema_missing() {
    // The daemon should provide clear error messages when schemas are missing
    let result = db::connect_and_verify(&["nonexistent_schema"]);
    
    assert!(
        result.is_err(),
        "Daemon should detect and report missing schemas"
    );
    
    if let Err(error) = result {
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("nonexistent_schema"),
            "Error message should identify the missing schema"
        );
    }
}

#[test]
fn test_daemon_loads_station_registry_on_startup() {
    // The daemon should load and validate station configuration from usgs_stations.toml
    let stations = stations::load_stations();
    
    assert!(
        !stations.is_empty(),
        "Station registry should contain configured monitoring stations"
    );
    
    // Verify key stations are present
    let site_codes: Vec<String> = stations.iter()
        .map(|s| s.site_code.clone())
        .collect();
    
    assert!(
        site_codes.contains(&"05568500".to_string()),
        "Registry should include Kingston Mines station"
    );
}

// ---------------------------------------------------------------------------
// 2. Data Staleness Detection
// ---------------------------------------------------------------------------

#[test]
fn test_daemon_detects_empty_database_as_stale() {
    let mut client = setup_test_db();
    cleanup_test_data(&mut client);
    
    // Query for most recent data for a test station
    let result = client.query(
        "SELECT MAX(reading_time) as latest FROM usgs_raw.gauge_readings WHERE site_code = $1",
        &[&"TEST0001"]
    ).expect("Query should succeed");
    
    assert!(
        result.is_empty() || result[0].get::<_, Option<DateTime<Utc>>>(0).is_none(),
        "Empty database should have no latest reading (staleness detected)"
    );
}

#[test]
fn test_daemon_calculates_staleness_from_latest_reading() {
    let mut client = setup_test_db();
    cleanup_test_data(&mut client);
    
    // Insert a reading from 2 hours ago
    let two_hours_ago = Utc::now() - Duration::hours(2);
    client.execute(
        "INSERT INTO usgs_raw.gauge_readings 
         (site_code, parameter_code, unit, value, reading_time, qualifier)
         VALUES ($1, $2, $3, $4, $5, $6)",
        &[
            &"TEST0001",
            &"00060",
            &"ft3/s",
            &Decimal::new(1000, 0),
            &two_hours_ago,
            &"P"
        ]
    ).expect("Insert should succeed");
    
    // Query for latest reading
    let rows = client.query(
        "SELECT reading_time FROM usgs_raw.gauge_readings 
         WHERE site_code = $1 
         ORDER BY reading_time DESC LIMIT 1",
        &[&"TEST0001"]
    ).expect("Query should succeed");
    
    assert!(!rows.is_empty(), "Should find the test reading");
    
    let latest: DateTime<Utc> = rows[0].get(0);
    let age_minutes = (Utc::now() - latest).num_minutes();
    
    assert!(
        age_minutes >= 119 && age_minutes <= 121,
        "Should detect reading is approximately 120 minutes old, got {} minutes",
        age_minutes
    );
    
    // Staleness threshold: readings older than 60 minutes are stale
    assert!(
        age_minutes > 60,
        "120-minute-old reading should exceed 60-minute staleness threshold"
    );
    
    cleanup_test_data(&mut client);
}

// ---------------------------------------------------------------------------
// 3. Backfill Missing Data
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Requires implementation of backfill logic
fn test_daemon_backfills_from_empty_database() {
    // When daemon starts with empty database, it should backfill historical data
    // This is a placeholder test that will guide implementation
    
    let mut client = setup_test_db();
    cleanup_test_data(&mut client);
    
    // TODO: Implement daemon.backfill_station() function
    // This should:
    // 1. Detect empty database (no readings for station)
    // 2. Fetch historical data (instantaneous values for last 120 days)
    // 3. Insert into usgs_raw.gauge_readings
    // 4. Update monitoring.station_state
    
    // let daemon = Daemon::new();
    // daemon.backfill_station("05568500").await?;
    
    // Verify backfill occurred
    // let count = client.query_one(
    //     "SELECT COUNT(*) FROM usgs_raw.gauge_readings WHERE site_code = $1",
    //     &[&"05568500"]
    // )?;
    
    // assert!(count.get::<_, i64>(0) > 0, "Backfill should populate database");
}

#[test]
#[ignore] // Requires implementation of gap detection
fn test_daemon_detects_and_fills_data_gaps() {
    // Daemon should detect gaps in time series and backfill them
    
    let mut client = setup_test_db();
    cleanup_test_data(&mut client);
    
    // Insert readings with a 24-hour gap
    let now = Utc::now();
    let yesterday = now - Duration::days(1);
    let three_days_ago = now - Duration::days(3);
    
    for reading_time in &[three_days_ago, yesterday, now] {
        client.execute(
            "INSERT INTO usgs_raw.gauge_readings 
             (site_code, parameter_code, unit, value, reading_time, qualifier)
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[
                &"TEST0001",
                &"00060",
                &"ft3/s",
                &Decimal::new(1000, 0),
                reading_time,
                &"P"
            ]
        ).expect("Insert should succeed");
    }
    
    // TODO: Implement gap detection
    // let gaps = daemon.detect_gaps("TEST0001", 15.minutes())?;
    // assert_eq!(gaps.len(), 1, "Should detect one 24-hour gap");
    
    // TODO: Implement gap filling
    // daemon.fill_gaps("TEST0001", &gaps).await?;
    
    cleanup_test_data(&mut client);
}

// ---------------------------------------------------------------------------
// 4. Continuous Monitoring and Warehousing
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Requires implementation of polling loop
fn test_daemon_polls_stations_on_schedule() {
    // Daemon should poll each station every 15 minutes (USGS update interval)
    
    // TODO: Implement daemon polling loop
    // let daemon = Daemon::new();
    // daemon.start_polling(interval_minutes: 15);
    
    // Verify polling configuration
    // assert_eq!(daemon.poll_interval(), Duration::minutes(15));
    
    // Verify all configured stations are in polling rotation
    // let polled_stations = daemon.polling_stations();
    // let configured_stations = stations::load_stations();
    // assert_eq!(polled_stations.len(), configured_stations.len());
}

#[test]
#[ignore] // Requires implementation of data warehousing
fn test_daemon_warehouses_new_readings() {
    // When daemon receives new data from USGS API, it should:
    // 1. Parse and validate the response
    // 2. Insert new readings into usgs_raw.gauge_readings
    // 3. Update monitoring.station_state with latest timestamp
    // 4. NOT insert duplicate readings (idempotent)
    
    let mut client = setup_test_db();
    cleanup_test_data(&mut client);
    
    // TODO: Implement daemon.poll_and_warehouse()
    // let initial_count = client.query_one(
    //     "SELECT COUNT(*) FROM usgs_raw.gauge_readings WHERE site_code = $1",
    //     &[&"TEST0001"]
    // )?;
    
    // daemon.poll_and_warehouse("TEST0001").await?;
    
    // let after_count = client.query_one(
    //     "SELECT COUNT(*) FROM usgs_raw.gauge_readings WHERE site_code = $1",
    //     &[&"TEST0001"]
    // )?;
    
    // assert!(after_count > initial_count, "Should insert new readings");
    
    cleanup_test_data(&mut client);
}

#[test]
#[ignore] // Requires implementation of duplicate prevention
fn test_daemon_prevents_duplicate_readings() {
    // Polling the same time period twice should not create duplicates
    
    let mut client = setup_test_db();
    cleanup_test_data(&mut client);
    
    // TODO: Implement idempotent warehousing
    // daemon.poll_and_warehouse("TEST0001").await?;
    // let first_count = get_reading_count(&client, "TEST0001");
    
    // Poll again with overlapping time range
    // daemon.poll_and_warehouse("TEST0001").await?;
    // let second_count = get_reading_count(&client, "TEST0001");
    
    // assert_eq!(first_count, second_count, "Should not insert duplicates");
    
    cleanup_test_data(&mut client);
}

#[test]
#[ignore] // Requires implementation of monitoring state
fn test_daemon_updates_monitoring_state() {
    // After successful polling, daemon should update monitoring.station_state
    
    let mut client = setup_test_db();
    cleanup_test_data(&mut client);
    
    // TODO: Implement monitoring state updates
    // daemon.poll_and_warehouse("TEST0001").await?;
    
    // let state = client.query_one(
    //     "SELECT latest_reading_time, last_poll_attempted, consecutive_failures 
    //      FROM usgs_raw.monitoring_state 
    //      WHERE site_code = $1",
    //     &[&"TEST0001"]
    // )?
    
    // assert!(state.get::<_, Option<DateTime<Utc>>>(0).is_some(), 
    //         "Should record last reading timestamp");
    // assert!(state.get::<_, Option<DateTime<Utc>>>(1).is_some(), 
    //         "Should record last poll timestamp");
    // assert_eq!(state.get::<_, i32>(2), 0, 
    //           "Successful poll should reset failure counter");
    
    cleanup_test_data(&mut client);
}

// ---------------------------------------------------------------------------
// 5. Error Handling and Resilience
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Requires implementation of error handling
fn test_daemon_handles_api_failures_gracefully() {
    // When USGS API is unreachable or returns errors:
    // 1. Log the failure
    // 2. Increment failure counter in monitoring.station_state
    // 3. Continue polling other stations
    // 4. Retry failed station on next cycle
    
    // TODO: Implement error handling
    // daemon.poll_and_warehouse("INVALID_SITE").await;
    
    // Verify failure was recorded
    // let state = get_monitoring_state(&client, "INVALID_SITE")?;
    // assert!(state.consecutive_failures > 0);
}

#[test]
#[ignore] // Requires implementation of alerts
fn test_daemon_alerts_on_excessive_staleness() {
    // When a station hasn't reported data for > 60 minutes:
    // 1. Generate staleness alert
    // 2. Continue monitoring (don't crash)
    // 3. Clear alert when fresh data arrives
    
    // TODO: Implement staleness alerting
}

// ---------------------------------------------------------------------------
// Helper Functions (for future use)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn get_reading_count(client: &mut Client, site_code: &str) -> i64 {
    client.query_one(
        "SELECT COUNT(*) FROM usgs_raw.gauge_readings WHERE site_code = $1",
        &[&site_code]
    )
    .expect("Query should succeed")
    .get(0)
}

#[allow(dead_code)]
fn get_latest_reading_time(client: &mut Client, site_code: &str) -> Option<DateTime<Utc>> {
    client.query_one(
        "SELECT MAX(reading_time) FROM usgs_raw.gauge_readings WHERE site_code = $1",
        &[&site_code]
    )
    .ok()
    .and_then(|row| row.get(0))
}
