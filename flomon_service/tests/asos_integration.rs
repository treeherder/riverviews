/// Integration tests for ASOS weather station data collection
///
/// Tests verify:
/// 1. ASOS data collection from IEM API
/// 2. Database storage in asos_observations table
/// 3. HTTP endpoint access for weather data
/// 4. Integration with daemon polling cycle
///
/// Prerequisites:
/// - PostgreSQL with asos_observations table (sql/006_iem_asos.sql)
/// - DATABASE_URL set in .env
/// - Internet access to mesonet.agron.iastate.edu
///
/// Run with: cargo test --test asos_integration -- --test-threads=1

use flomon_service::asos_locations;
use flomon_service::ingest::iem;
use postgres::{Client, NoTls};
use chrono::Utc;
use std::env;

// ---------------------------------------------------------------------------
// Test Helpers
// ---------------------------------------------------------------------------

fn setup_test_db() -> Client {
    dotenv::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut client = Client::connect(&database_url, NoTls).expect("Failed to connect to test database");
    
    // Ensure test station exists (required for foreign key constraint)
    let _ = client.execute(
        "INSERT INTO asos_stations 
         (station_id, name, latitude, longitude, elevation_ft, basin, priority, 
          poll_interval_minutes, data_types, relevance)
         VALUES 
         ('TESTSTA', 'Test Station', 40.0, -89.0, 500.0, 'Test Basin', 'LOW', 
          60, ARRAY['temperature', 'precipitation'], 'Test station for integration tests'),
         ('TESTPEORIA', 'Test Peoria', 40.6642, -89.6931, 652.0, 'Illinois', 'LOW',
          60, ARRAY['precipitation'], 'Test Peoria station'),
         ('TESTCHICAGO', 'Test Chicago', 41.9742, -87.9073, 672.0, 'Illinois', 'LOW',
          60, ARRAY['precipitation'], 'Test Chicago station'),
         ('TESTSPRINGFIELD', 'Test Springfield', 39.8436, -89.6779, 597.0, 'Illinois', 'LOW',
          60, ARRAY['precipitation'], 'Test Springfield station'),
         ('TESTSTA1', 'Test Station 1', 40.0, -89.0, 500.0, 'Test', 'LOW',
          60, ARRAY['precipitation'], 'Test station 1'),
         ('TESTSTA2', 'Test Station 2', 40.1, -89.1, 510.0, 'Test', 'LOW',
          60, ARRAY['precipitation'], 'Test station 2'),
         ('TESTSTA3', 'Test Station 3', 40.2, -89.2, 520.0, 'Test', 'LOW',
          60, ARRAY['precipitation'], 'Test station 3')
         ON CONFLICT (station_id) DO NOTHING",
        &[]
    );
    
    // Also ensure real stations exist for API tests
    let _ = client.execute(
        "INSERT INTO asos_stations 
         (station_id, name, latitude, longitude, elevation_ft, basin, priority, 
          poll_interval_minutes, data_types, relevance)
         VALUES 
         ('PIA', 'Peoria Greater Peoria Airport', 40.6642, -89.6931, 652.0, 'Illinois', 'CRITICAL',
          15, ARRAY['temperature', 'precipitation', 'wind', 'pressure'], 'Primary local precipitation station')
         ON CONFLICT (station_id) DO NOTHING",
        &[]
    );
    
    client
}

fn cleanup_asos_test_data(client: &mut Client) {
    // Clean up test data (but keep test stations for foreign key constraints)
    let _ = client.execute("DELETE FROM asos_observations WHERE station_id LIKE 'TEST%'", &[]);
}

fn get_asos_observation_count(client: &mut Client, station_id: &str) -> i64 {
    client
        .query_one(
            "SELECT COUNT(*) FROM asos_observations WHERE station_id = $1",
            &[&station_id],
        )
        .map(|row| row.get(0))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 1. ASOS Data Collection Tests
// ---------------------------------------------------------------------------

#[test]
fn test_asos_station_configuration_loads() {
    // Verify ASOS station configuration can be loaded
    let result = asos_locations::load_locations("./iem_asos.toml");
    
    assert!(
        result.is_ok(),
        "Should successfully load ASOS station configuration"
    );
    
    let stations = result.unwrap();
    assert!(
        stations.len() > 0,
        "Should have at least one ASOS station configured"
    );
    
    // Verify primary station (KPIA - Peoria) is configured
    let peoria = stations.iter().find(|s| s.station_id == "KPIA");
    assert!(
        peoria.is_some(),
        "KPIA (Peoria Airport) should be in configuration"
    );
}

#[test]
fn test_asos_api_fetches_current_observations() {
    // Test direct API call to IEM ASOS endpoint
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    
    let result = iem::fetch_recent_precip(&client, "KPIA", 4);
    
    assert!(
        result.is_ok(),
        "Should successfully fetch ASOS data from IEM API: {:?}",
        result.err()
    );
    
    let observations = result.unwrap();
    assert!(
        observations.len() > 0,
        "Should receive at least some observations from KPIA"
    );
    
    // Verify observation structure
    let first_obs = &observations[0];
    assert_eq!(first_obs.station_id, "PIA", "Station ID should be normalized");
    assert!(first_obs.timestamp <= Utc::now(), "Timestamp should be in the past");
}

#[test]
fn test_asos_observations_parse_correctly() {
    // Test that ASOS observations parse all expected fields
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    
    let observations = iem::fetch_recent_precip(&client, "KPIA", 4).unwrap();
    
    let mut has_temp = false;
    let mut has_precip = false;
    let mut has_wind = false;
    let mut has_pressure = false;
    
    for obs in &observations {
        if obs.temp_f.is_some() { has_temp = true; }
        if obs.precip_1hr_in.is_some() { has_precip = true; }
        if obs.wind_speed_knots.is_some() { has_wind = true; }
        if obs.pressure_mb.is_some() { has_pressure = true; }
    }
    
    assert!(has_temp, "Should parse temperature values");
    // Note: precip, wind, pressure may be null in some observations
    
    println!("Parsed {} observations", observations.len());
    println!("  Temperature: {}", if has_temp { "✓" } else { "✗" });
    println!("  Precipitation: {}", if has_precip { "✓" } else { "✗" });
    println!("  Wind: {}", if has_wind { "✓" } else { "✗" });
    println!("  Pressure: {}", if has_pressure { "✓" } else { "✗" });
}

// ---------------------------------------------------------------------------
// 2. Database Storage Tests
// ---------------------------------------------------------------------------

#[test]
fn test_asos_schema_exists() {
    // Verify asos_observations table exists with correct schema
    let mut client = setup_test_db();
    
    let result = client.query(
        "SELECT column_name, data_type 
         FROM information_schema.columns 
         WHERE table_name = 'asos_observations'
         ORDER BY ordinal_position",
        &[]
    );
    
    assert!(result.is_ok(), "asos_observations table should exist");
    
    let columns = result.unwrap();
    let column_names: Vec<String> = columns
        .iter()
        .map(|row| row.get(0))
        .collect();
    
    // Verify key columns exist
    assert!(column_names.contains(&"station_id".to_string()));
    assert!(column_names.contains(&"observation_time".to_string()));
    assert!(column_names.contains(&"temp_f".to_string()));
    assert!(column_names.contains(&"precip_1hr_in".to_string()));
    assert!(column_names.contains(&"wind_speed_knots".to_string()));
    assert!(column_names.contains(&"pressure_mb".to_string()));
    
    println!("Found {} columns in asos_observations table", columns.len());
}

#[test]
fn test_asos_observations_can_be_stored() {
    // Test inserting ASOS observations into database
    let mut client = setup_test_db();
    cleanup_asos_test_data(&mut client);
    
    // Clear any existing PIA data from previous tests
    let _ = client.execute("DELETE FROM asos_observations WHERE station_id = 'PIA'", &[]);
    
    // Fetch real observations
    let http_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    
    let observations = iem::fetch_recent_precip(&http_client, "KPIA", 1).unwrap();
    assert!(observations.len() > 0, "Should have observations to test with");
    
    println!("Fetched {} observations from IEM", observations.len());
    
    // Insert observations
    let mut inserted = 0;
    let mut errors = 0;
    for obs in &observations {
        let result = client.execute(
            "INSERT INTO asos_observations 
             (station_id, observation_time, temp_f, dewpoint_f, relative_humidity,
              wind_direction_deg, wind_speed_knots, wind_gust_knots, precip_1hr_in,
              pressure_mb, visibility_mi, sky_condition, weather_codes, data_source)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'TEST')
             ON CONFLICT (station_id, observation_time) DO NOTHING",
            &[
                &obs.station_id,
                &obs.timestamp,
                &obs.temp_f,
                &obs.dewpoint_f,
                &obs.relative_humidity,
                &obs.wind_direction_deg,
                &obs.wind_speed_knots,
                &obs.wind_gust_knots,
                &obs.precip_1hr_in,
                &obs.pressure_mb,
                &obs.visibility_mi,
                &obs.sky_condition,
                &obs.weather_codes,
            ]
        );
        
        match result {
            Ok(rows) => {
                if rows > 0 {
                    inserted += 1;
                }
            }
            Err(e) => {
                println!("Insert error: {}", e);
                errors += 1;
            }
        }
    }
    
    println!("Inserted: {}, Errors: {}, Total: {}", inserted, errors, observations.len());
    
    assert!(inserted > 0, "Should successfully insert at least one observation");
    assert_eq!(errors, 0, "Should not have any insert errors");
    
    // Verify data was stored
    let count = get_asos_observation_count(&mut client, "PIA");
    assert!(count > 0, "Should find stored observations in database");
    
    println!("Successfully stored {} ASOS observations", inserted);
    
    // Cleanup
    let _ = client.execute("DELETE FROM asos_observations WHERE station_id = 'PIA'", &[]);
    cleanup_asos_test_data(&mut client);
}

#[test]
fn test_asos_duplicate_observations_handled() {
    // Test that duplicate observations are handled gracefully (ON CONFLICT)
    let mut client = setup_test_db();
    cleanup_asos_test_data(&mut client);
    
    let http_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    
    let observations = iem::fetch_recent_precip(&http_client, "KPIA", 1).unwrap();
    let obs = &observations[0];
    
    // Insert same observation twice
    let insert_sql = "INSERT INTO asos_observations 
         (station_id, observation_time, temp_f, data_source)
         VALUES ($1, $2, $3, 'TEST')
         ON CONFLICT (station_id, observation_time) DO NOTHING";
    
    let first_insert = client.execute(insert_sql, &[&obs.station_id, &obs.timestamp, &obs.temp_f]);
    let second_insert = client.execute(insert_sql, &[&obs.station_id, &obs.timestamp, &obs.temp_f]);
    
    assert!(first_insert.is_ok(), "First insert should succeed");
    assert!(second_insert.is_ok(), "Second insert should not error (ON CONFLICT)");
    assert_eq!(second_insert.unwrap(), 0, "Second insert should affect 0 rows");
    
    cleanup_asos_test_data(&mut client);
}

// ---------------------------------------------------------------------------
// 3. HTTP Endpoint Tests
// ---------------------------------------------------------------------------

#[test]
fn test_asos_appears_in_zone_sensors() {
    // Test that ASOS sensors appear in zone detail responses
    // Note: This test verifies the data structure, actual endpoint requires running daemon
    use flomon_service::zones::Sensor;
    
    // Verify is_asos() method works correctly
    let asos_sensor = Sensor {
        sensor_id: Some("WEATHER001".to_string()),
        usgs_id: None,
        station_id: Some("KPIA".to_string()),
        cwms_location: None,
        shef_id: None,
        source: "IEM/ASOS".to_string(),
        sensor_type: "precipitation".to_string(),
        role: "precip".to_string(),
        location: "Peoria Airport Weather Station".to_string(),
        lat: 40.6642,
        lon: -89.6931,
        relevance: "Provides precipitation data for basin analysis".to_string(),
        pool_target_ft_ngvd29: None,
        flood_stage_ft: None,
        action_stage_ft: None,
        moderate_flood_ft: None,
        major_flood_ft: None,
        datum_note: None,
    };
    
    assert!(asos_sensor.is_asos(), "Weather sensor should be identified as ASOS");
    assert_eq!(asos_sensor.source, "IEM/ASOS", "Source should be IEM/ASOS");
}

#[test]
fn test_asos_data_queryable_from_database() {
    // Test querying ASOS data as endpoint would
    let mut client = setup_test_db();
    cleanup_asos_test_data(&mut client);
    
    let now = Utc::now();
    let station_id = "TESTSTA";
    
    // Insert test observation
    let _ = client.execute(
        "INSERT INTO asos_observations 
         (station_id, observation_time, precip_1hr_in, temp_f, data_source)
         VALUES ($1, $2, 0.25, 72.5, 'TEST')",
        &[&station_id, &now]
    );
    
    // Query like endpoint does
    let rows = client.query(
        "SELECT precip_1hr_in, observation_time
         FROM asos_observations
         WHERE station_id = $1
         ORDER BY observation_time DESC
         LIMIT 1",
        &[&station_id]
    ).unwrap();
    
    assert_eq!(rows.len(), 1, "Should find most recent observation");
    
    let row = &rows[0];
    let precip: Option<f64> = row.get(0);
    let timestamp: chrono::DateTime<Utc> = row.get(1);
    
    assert_eq!(precip, Some(0.25), "Should retrieve precipitation value");
    assert!((timestamp - now).num_seconds().abs() < 2, "Timestamp should match");
    
    cleanup_asos_test_data(&mut client);
}

// ---------------------------------------------------------------------------
// 4. Daemon Integration Tests
// ---------------------------------------------------------------------------

#[test]
fn test_asos_configuration_loads_in_daemon() {
    // Test that ASOS configuration loads without errors
    let result = asos_locations::load_locations("./iem_asos.toml");
    
    assert!(result.is_ok(), "ASOS configuration should load successfully");
    
    let locations = result.unwrap();
    println!("Loaded {} ASOS stations for daemon", locations.len());
    
    // Verify at least some expected stations
    let station_ids: Vec<String> = locations
        .iter()
        .map(|l| l.station_id.clone())
        .collect();
    
    assert!(station_ids.contains(&"KPIA".to_string()), "Should include KPIA");
}

#[test]
fn test_asos_data_freshness_check() {
    // Test checking staleness of ASOS observations
    let mut client = setup_test_db();
    
    // Insert a recent observation
    let now = Utc::now();
    let _ = client.execute(
        "INSERT INTO asos_observations (station_id, observation_time, temp_f, data_source)
         VALUES ('TESTSTA', $1, 72.5, 'TEST')
         ON CONFLICT (station_id, observation_time) DO NOTHING",
        &[&now]
    );
    
    // Check for most recent observation
    let result = client.query_one(
        "SELECT observation_time FROM asos_observations 
         WHERE station_id = 'TESTSTA' 
         ORDER BY observation_time DESC LIMIT 1",
        &[]
    );
    
    assert!(result.is_ok(), "Should find most recent observation");
    
    cleanup_asos_test_data(&mut client);
}

// ---------------------------------------------------------------------------
// 4. Weather Data Queries
// ---------------------------------------------------------------------------

#[test]
fn test_query_recent_precipitation() {
    // Test querying recent precipitation totals
    let mut client = setup_test_db();
    cleanup_asos_test_data(&mut client);
    
    // Insert test observations with precipitation
    let now = Utc::now();
    let _ = client.execute(
        "INSERT INTO asos_observations 
         (station_id, observation_time, temp_f, precip_1hr_in, data_source)
         VALUES 
         ('TESTSTA', $1, 72.0, 0.05, 'TEST'),
         ('TESTSTA', $2, 71.5, 0.10, 'TEST'),
         ('TESTSTA', $3, 71.0, 0.02, 'TEST')",
        &[
            &(now - chrono::Duration::hours(2)),
            &(now - chrono::Duration::hours(1)),
            &now
        ]
    );
    
    // Query recent precipitation
    let result = client.query(
        "SELECT SUM(precip_1hr_in) as total_precip
         FROM asos_observations
         WHERE station_id = 'TESTSTA'
         AND observation_time > $1",
        &[&(now - chrono::Duration::hours(3))]
    );
    
    assert!(result.is_ok(), "Should calculate precipitation totals");
    
    cleanup_asos_test_data(&mut client);
}

#[test]
fn test_query_temperature_trends() {
    // Test querying temperature trends over time
    let mut client = setup_test_db();
    cleanup_asos_test_data(&mut client);
    
    let now = Utc::now();
    
    // Insert temperature observations
    let _ = client.execute(
        "INSERT INTO asos_observations 
         (station_id, observation_time, temp_f, data_source)
         VALUES 
         ('TESTSTA', $1, 68.0, 'TEST'),
         ('TESTSTA', $2, 72.0, 'TEST'),
         ('TESTSTA', $3, 75.0, 'TEST')",
        &[
            &(now - chrono::Duration::hours(2)),
            &(now - chrono::Duration::hours(1)),
            &now
        ]
    );
    
    // Query temperature change
    let result = client.query_one(
        "SELECT MAX(temp_f) - MIN(temp_f) as temp_change
         FROM asos_observations
         WHERE station_id = 'TESTSTA'",
        &[]
    );
    
    assert!(result.is_ok(), "Should calculate temperature trends");
    
    if let Ok(row) = result {
        let temp_change: Option<f64> = row.get(0);
        assert_eq!(temp_change, Some(7.0), "Temperature change should be 7°F");
    }
    
    cleanup_asos_test_data(&mut client);
}

// ---------------------------------------------------------------------------
// 5. Multi-Station Queries
// ---------------------------------------------------------------------------

#[test]
fn test_query_multiple_asos_stations() {
    // Test querying data from multiple stations simultaneously
    let mut client = setup_test_db();
    cleanup_asos_test_data(&mut client);
    
    let now = Utc::now();
    
    // Insert observations for multiple test stations
    let _ = client.execute(
        "INSERT INTO asos_observations 
         (station_id, observation_time, temp_f, precip_1hr_in, data_source)
         VALUES 
         ('TESTPEORIA', $1, 72.0, 0.05, 'TEST'),
         ('TESTCHICAGO', $1, 68.0, 0.10, 'TEST'),
         ('TESTSPRINGFIELD', $1, 75.0, 0.00, 'TEST')",
        &[&now]
    );
    
    // Query all stations
    let result = client.query(
        "SELECT station_id, temp_f, precip_1hr_in
         FROM asos_observations
         WHERE station_id LIKE 'TEST%'
         AND observation_time = $1
         ORDER BY station_id",
        &[&now]
    );
    
    assert!(result.is_ok(), "Should query multiple stations");
    
    let rows = result.unwrap();
    assert_eq!(rows.len(), 3, "Should return data for all 3 test stations");
    
    cleanup_asos_test_data(&mut client);
}

#[test]
fn test_basin_wide_precipitation_summary() {
    // Test generating basin-wide precipitation summary
    let mut client = setup_test_db();
    cleanup_asos_test_data(&mut client);
    
    let now = Utc::now();
    
    // Insert precipitation data for multiple stations
    let _ = client.execute(
        "INSERT INTO asos_observations 
         (station_id, observation_time, precip_1hr_in, data_source)
         VALUES 
         ('TESTSTA1', $1, 0.25, 'TEST'),
         ('TESTSTA2', $1, 0.50, 'TEST'),
         ('TESTSTA3', $1, 0.10, 'TEST')",
        &[&now]
    );
    
    // Calculate basin-wide statistics
    let result = client.query_one(
        "SELECT 
            COUNT(*) as station_count,
            AVG(precip_1hr_in) as avg_precip,
            MAX(precip_1hr_in) as max_precip,
            MIN(precip_1hr_in) as min_precip
         FROM asos_observations
         WHERE station_id LIKE 'TEST%'
         AND observation_time = $1",
        &[&now]
    );
    
    assert!(result.is_ok(), "Should calculate basin statistics");
    
    if let Ok(row) = result {
        let station_count: i64 = row.get(0);
        assert_eq!(station_count, 3, "Should count all 3 stations");
        
        let avg_precip: Option<f64> = row.get(1);
        assert!(avg_precip.is_some(), "Should calculate average precipitation");
    }
    
    cleanup_asos_test_data(&mut client);
}

// ---------------------------------------------------------------------------
// 6. Data Quality Tests
// ---------------------------------------------------------------------------

#[test]
fn test_asos_handles_null_values() {
    // Test that null/missing values are handled correctly
    let mut client = setup_test_db();
    cleanup_asos_test_data(&mut client);
    
    let now = Utc::now();
    
    // Insert observation with some null values
    let result = client.execute(
        "INSERT INTO asos_observations 
         (station_id, observation_time, temp_f, precip_1hr_in, data_source)
         VALUES ('TESTSTA', $1, 72.0, NULL, 'TEST')",
        &[&now]
    );
    
    assert!(result.is_ok(), "Should handle NULL values in observations");
    
    // Query and verify
    let query_result = client.query_one(
        "SELECT temp_f, precip_1hr_in FROM asos_observations WHERE station_id = 'TESTSTA'",
        &[]
    );
    
    assert!(query_result.is_ok());
    
    if let Ok(row) = query_result {
        let temp: Option<f64> = row.get(0);
        let precip: Option<f64> = row.get(1);
        
        assert_eq!(temp, Some(72.0), "Temperature should be stored");
        assert_eq!(precip, None, "Precipitation should be NULL");
    }
    
    cleanup_asos_test_data(&mut client);
}

#[test]
fn test_asos_timestamp_ordering() {
    // Test that observations are properly ordered by timestamp
    let mut client = setup_test_db();
    cleanup_asos_test_data(&mut client);
    
    let now = Utc::now();
    
    // Insert observations in random order
    let _ = client.execute(
        "INSERT INTO asos_observations 
         (station_id, observation_time, temp_f, data_source)
         VALUES 
         ('TESTSTA', $1, 70.0, 'TEST'),
         ('TESTSTA', $2, 72.0, 'TEST'),
         ('TESTSTA', $3, 68.0, 'TEST')",
        &[
            &(now - chrono::Duration::hours(1)),
            &now,
            &(now - chrono::Duration::hours(2))
        ]
    );
    
    // Query in timestamp order
    let result = client.query(
        "SELECT temp_f FROM asos_observations 
         WHERE station_id = 'TESTSTA'
         ORDER BY observation_time ASC",
        &[]
    );
    
    assert!(result.is_ok());
    
    let rows = result.unwrap();
    let temps: Vec<f64> = rows.iter().map(|r| r.get(0)).collect();
    
    assert_eq!(temps, vec![68.0, 70.0, 72.0], "Should be ordered chronologically");
    
    cleanup_asos_test_data(&mut client);
}
