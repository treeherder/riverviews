#!/usr/bin/env python3
"""
Generate zone-based snapshots for historical flood events.

This script creates a "moment in time" report for each known historical flood,
showing the status of all 7 hydrological zones at the time of each flood peak.

This data is used for:
1. Regression analysis to identify flood precursors
2. Understanding which zones were active during major floods
3. Classifying historical events (top-down, bottom-up, compound, local tributary)
4. Training ML models to predict flood arrival times

Usage:
    python3 scripts/generate_flood_zone_snapshots.py [--output PEAK_FLOW_SUMMARY.md]

Prerequisites:
    - PostgreSQL database with flood events populated (nws.flood_events)
    - Historical gauge readings in usgs_raw.gauge_readings
    - zones.toml configuration file
"""

import argparse
import sys
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass
import psycopg2
from psycopg2.extras import RealDictCursor
import toml


@dataclass
class FloodEvent:
    """Historical flood event"""
    event_id: int
    site_code: str
    crest_time: datetime
    peak_stage_ft: float
    severity: str
    event_name: Optional[str] = None


@dataclass
class SensorReading:
    """Sensor reading at a specific time"""
    sensor_id: str
    sensor_type: str
    value: float
    unit: str
    timestamp: datetime
    source: str  # 'USGS', 'CWMS', 'ASOS'


@dataclass
class ZoneSnapshot:
    """Complete zone status at a moment in time"""
    zone_id: int
    zone_name: str
    snapshot_time: datetime
    sensors: List[Tuple[str, Optional[SensorReading]]]  # (sensor_id, reading)
    zone_status: str  # 'NORMAL', 'ELEVATED', 'WARNING', 'CRITICAL'


@dataclass
class FloodEventSnapshot:
    """Complete basin snapshot for a flood event"""
    event: FloodEvent
    zones: List[ZoneSnapshot]
    event_classification: str  # 'TOP_DOWN', 'BOTTOM_UP', 'LOCAL_TRIBUTARY', 'COMPOUND'
    backwater_active: bool
    upstream_pulse_detected: bool
    local_tributary_active: bool


class ZoneSnapshotGenerator:
    """Generate zone-based snapshots for historical floods"""
    
    def __init__(self, db_url: str, zones_config_path: str):
        self.db_url = db_url
        self.zones_config_path = zones_config_path
        self.zones_config = None
        self.conn = None
    
    def connect(self):
        """Connect to database"""
        self.conn = psycopg2.connect(self.db_url)
        
    def load_zones_config(self):
        """Load zones.toml configuration"""
        with open(self.zones_config_path, 'r') as f:
            self.zones_config = toml.load(f)
    
    def fetch_historical_flood_events(self) -> List[FloodEvent]:
        """Fetch all historical flood events from database"""
        with self.conn.cursor(cursor_factory=RealDictCursor) as cur:
            cur.execute("""
                SELECT 
                    id,
                    site_code,
                    crest_time,
                    peak_stage_ft,
                    severity,
                    event_name
                FROM nws.flood_events
                WHERE crest_time IS NOT NULL
                AND severity IN ('flood', 'moderate', 'major')
                ORDER BY crest_time DESC
            """)
            
            events = []
            for row in cur.fetchall():
                events.append(FloodEvent(
                    event_id=row['id'],
                    site_code=row['site_code'],
                    crest_time=row['crest_time'],
                    peak_stage_ft=float(row['peak_stage_ft']),
                    severity=row['severity'],
                    event_name=row['event_name']
                ))
            
            return events
    
    def fetch_sensor_reading(
        self,
        sensor_id: str,
        sensor_type: str,
        source: str,
        target_time: datetime,
        window_hours: int = 6
    ) -> Optional[SensorReading]:
        """
        Fetch sensor reading closest to target time (within window).
        
        Args:
            sensor_id: USGS site code, CWMS location, or ASOS station ID
            sensor_type: 'stage', 'discharge', 'pool', 'precipitation', etc.
            source: 'USGS', 'CWMS', or 'ASOS'
            target_time: Desired timestamp
            window_hours: Search window (±hours from target)
        
        Returns:
            SensorReading or None if no data available
        """
        start_time = target_time - timedelta(hours=window_hours)
        end_time = target_time + timedelta(hours=window_hours)
        
        if source == 'USGS':
            # Query USGS gauge readings
            parameter_code = '00065' if sensor_type == 'stage' else '00060'
            
            with self.conn.cursor(cursor_factory=RealDictCursor) as cur:
                cur.execute("""
                    SELECT 
                        site_code,
                        parameter_code,
                        value,
                        unit,
                        reading_time
                    FROM usgs_raw.gauge_readings
                    WHERE site_code = %s
                    AND parameter_code = %s
                    AND reading_time BETWEEN %s AND %s
                    ORDER BY ABS(EXTRACT(EPOCH FROM (reading_time - %s)))
                    LIMIT 1
                """, (sensor_id, parameter_code, start_time, end_time, target_time))
                
                row = cur.fetchone()
                if row:
                    return SensorReading(
                        sensor_id=sensor_id,
                        sensor_type=sensor_type,
                        value=float(row['value']),
                        unit=row['unit'],
                        timestamp=row['reading_time'],
                        source='USGS'
                    )
        
        elif source == 'CWMS':
            # Query CWMS timeseries
            with self.conn.cursor(cursor_factory=RealDictCursor) as cur:
                cur.execute("""
                    SELECT 
                        location_id,
                        value,
                        unit,
                        timestamp
                    FROM usace.cwms_timeseries
                    WHERE location_id LIKE %s
                    AND timestamp BETWEEN %s AND %s
                    ORDER BY ABS(EXTRACT(EPOCH FROM (timestamp - %s)))
                    LIMIT 1
                """, (f'%{sensor_id}%', start_time, end_time, target_time))
                
                row = cur.fetchone()
                if row:
                    return SensorReading(
                        sensor_id=sensor_id,
                        sensor_type=sensor_type,
                        value=float(row['value']),
                        unit=row['unit'],
                        timestamp=row['timestamp'],
                        source='CWMS'
                    )
        
        elif source == 'ASOS':
            # Query ASOS observations
            with self.conn.cursor(cursor_factory=RealDictCursor) as cur:
                cur.execute("""
                    SELECT 
                        station_id,
                        precip_6hr_in,
                        observation_time
                    FROM asos_observations
                    WHERE station_id = %s
                    AND observation_time BETWEEN %s AND %s
                    ORDER BY ABS(EXTRACT(EPOCH FROM (observation_time - %s)))
                    LIMIT 1
                """, (sensor_id, start_time, end_time, target_time))
                
                row = cur.fetchone()
                if row and row['precip_6hr_in'] is not None:
                    return SensorReading(
                        sensor_id=sensor_id,
                        sensor_type='precipitation',
                        value=float(row['precip_6hr_in']),
                        unit='inches',
                        timestamp=row['observation_time'],
                        source='ASOS'
                    )
        
        return None
    
    def generate_zone_snapshot(
        self,
        zone_id: int,
        target_time: datetime
    ) -> ZoneSnapshot:
        """Generate zone snapshot at specific time"""
        if not self.zones_config:
            raise RuntimeError("zones.toml not loaded")
        
        zone = self.zones_config['zones'][str(zone_id)]
        zone_name = zone['name']
        
        sensor_readings = []
        elevated_count = 0
        critical_count = 0
        
        for sensor in zone['sensors']:
            # Determine sensor source and ID
            if 'usgs_id' in sensor:
                sensor_id = sensor['usgs_id']
                source = 'USGS'
            elif 'cwms_location' in sensor:
                sensor_id = sensor['cwms_location']
                source = 'CWMS'
            elif 'station_id' in sensor:
                sensor_id = sensor['station_id']
                source = 'ASOS'
            else:
                sensor_id = sensor.get('shef_id', 'UNKNOWN')
                source = 'UNKNOWN'
            
            sensor_type = sensor['sensor_type']
            
            # Fetch reading closest to target time
            reading = self.fetch_sensor_reading(
                sensor_id, sensor_type, source, target_time
            )
            
            sensor_readings.append((sensor_id, reading))
            
            # Check if sensor is elevated (simple threshold check)
            if reading:
                if sensor_type in ['stage', 'pool_elevation']:
                    action_stage = sensor.get('action_stage_ft')
                    flood_stage = sensor.get('flood_stage_ft')
                    
                    if flood_stage and reading.value >= flood_stage:
                        critical_count += 1
                    elif action_stage and reading.value >= action_stage:
                        elevated_count += 1
        
        # Determine zone status
        if critical_count > 0:
            zone_status = 'CRITICAL'
        elif elevated_count > 0:
            zone_status = 'WARNING'
        elif len([r for _, r in sensor_readings if r is not None]) < len(sensor_readings) / 2:
            zone_status = 'DEGRADED'
        else:
            zone_status = 'NORMAL'
        
        return ZoneSnapshot(
            zone_id=zone_id,
            zone_name=zone_name,
            snapshot_time=target_time,
            sensors=sensor_readings,
            zone_status=zone_status
        )
    
    def classify_flood_event(
        self,
        zone_snapshots: List[ZoneSnapshot]
    ) -> Tuple[str, bool, bool, bool]:
        """
        Classify flood event type based on zone activity.
        
        Returns:
            (event_type, backwater_active, upstream_pulse, local_tributary)
        """
        zone_statuses = {z.zone_id: z.zone_status for z in zone_snapshots}
        
        # Check zone activity
        zone_0_active = zone_statuses.get(0) in ['WARNING', 'CRITICAL']  # Mississippi
        zone_1_active = zone_statuses.get(1) in ['WARNING', 'CRITICAL']  # LaGrange
        zone_3_active = zone_statuses.get(3) in ['WARNING', 'CRITICAL']  # Tributaries
        zone_4_plus_active = any(
            zone_statuses.get(z) in ['WARNING', 'CRITICAL'] 
            for z in [4, 5, 6]
        )
        
        backwater_active = zone_0_active or zone_1_active
        local_tributary_active = zone_3_active
        upstream_pulse_detected = zone_4_plus_active
        
        # Classify event type
        if backwater_active and upstream_pulse_detected:
            event_type = 'COMPOUND'
        elif backwater_active:
            event_type = 'BOTTOM_UP'
        elif local_tributary_active and not upstream_pulse_detected:
            event_type = 'LOCAL_TRIBUTARY'
        elif upstream_pulse_detected:
            event_type = 'TOP_DOWN'
        else:
            event_type = 'UNKNOWN'
        
        return event_type, backwater_active, upstream_pulse_detected, local_tributary_active
    
    def generate_flood_snapshot(
        self,
        event: FloodEvent
    ) -> FloodEventSnapshot:
        """Generate complete basin snapshot for flood event"""
        # Generate snapshots for all 7 zones
        zone_snapshots = []
        for zone_id in range(7):
            snapshot = self.generate_zone_snapshot(zone_id, event.crest_time)
            zone_snapshots.append(snapshot)
        
        # Classify event
        event_type, backwater, upstream, tributary = self.classify_flood_event(
            zone_snapshots
        )
        
        return FloodEventSnapshot(
            event=event,
            zones=zone_snapshots,
            event_classification=event_type,
            backwater_active=backwater,
            upstream_pulse_detected=upstream,
            local_tributary_active=tributary
        )
    
    def generate_all_snapshots(self) -> List[FloodEventSnapshot]:
        """Generate snapshots for all historical floods"""
        events = self.fetch_historical_flood_events()
        snapshots = []
        
        print(f"Found {len(events)} historical flood events")
        
        for event in events:
            print(f"  Processing {event.site_code} on {event.crest_time.date()}...")
            snapshot = self.generate_flood_snapshot(event)
            snapshots.append(snapshot)
        
        return snapshots
    
    def format_markdown_report(self, snapshots: List[FloodEventSnapshot]) -> str:
        """Format snapshots as markdown report"""
        lines = []
        
        lines.append("# Historical Flood Event Zone Analysis")
        lines.append("")
        lines.append(f"**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        lines.append(f"**Events Analyzed:** {len(snapshots)}")
        lines.append("")
        lines.append("This report shows the status of all 7 hydrological zones at the moment of each")
        lines.append("known historical flood crest. This data provides ground truth for regression")
        lines.append("analysis and flood forecasting model training.")
        lines.append("")
        
        # Summary statistics
        event_types = {}
        for snapshot in snapshots:
            event_types[snapshot.event_classification] = event_types.get(
                snapshot.event_classification, 0
            ) + 1
        
        lines.append("## Event Type Distribution")
        lines.append("")
        lines.append("| Event Type | Count | Percentage |")
        lines.append("|------------|-------|------------|")
        for event_type, count in sorted(event_types.items(), key=lambda x: -x[1]):
            pct = (count / len(snapshots)) * 100
            lines.append(f"| {event_type} | {count} | {pct:.1f}% |")
        lines.append("")
        
        # Individual flood events
        lines.append("## Individual Flood Events")
        lines.append("")
        
        for snapshot in snapshots:
            event = snapshot.event
            lines.append(f"### {event.site_code} – {event.crest_time.strftime('%Y-%m-%d %H:%M')}")
            lines.append("")
            lines.append(f"**Severity:** {event.severity.upper()}  ")
            lines.append(f"**Peak Stage:** {event.peak_stage_ft:.2f} ft  ")
            lines.append(f"**Event Type:** {snapshot.event_classification}  ")
            
            if event.event_name:
                lines.append(f"**Event Name:** {event.event_name}  ")
            
            lines.append("")
            
            # Zone status table
            lines.append("#### Zone Status at Crest")
            lines.append("")
            lines.append("| Zone | Name | Status | Active Sensors | Data Coverage |")
            lines.append("|------|------|--------|----------------|---------------|")
            
            for zone in snapshot.zones:
                total_sensors = len(zone.sensors)
                active_sensors = len([r for _, r in zone.sensors if r is not None])
                coverage_pct = (active_sensors / total_sensors * 100) if total_sensors > 0 else 0
                
                lines.append(
                    f"| {zone.zone_id} | {zone.zone_name[:30]}... | "
                    f"{zone.zone_status} | {active_sensors}/{total_sensors} | "
                    f"{coverage_pct:.0f}% |"
                )
            
            lines.append("")
            
            # Event classification details
            lines.append("#### Event Characteristics")
            lines.append("")
            lines.append(f"- **Backwater Active:** {'Yes' if snapshot.backwater_active else 'No'}")
            lines.append(f"- **Upstream Pulse:** {'Yes' if snapshot.upstream_pulse_detected else 'No'}")
            lines.append(f"- **Local Tributary:** {'Yes' if snapshot.local_tributary_active else 'No'}")
            lines.append("")
            
            # Key sensor readings
            lines.append("#### Critical Sensor Readings")
            lines.append("")
            
            # Zone 2 (property zone) - most important
            zone_2 = next((z for z in snapshot.zones if z.zone_id == 2), None)
            if zone_2:
                lines.append("**Zone 2 (Property Zone):**")
                for sensor_id, reading in zone_2.sensors[:3]:  # Top 3 sensors
                    if reading:
                        lines.append(
                            f"- {sensor_id}: {reading.value:.2f} {reading.unit} "
                            f"({reading.timestamp.strftime('%H:%M')})"
                        )
                    else:
                        lines.append(f"- {sensor_id}: NO DATA")
                lines.append("")
            
            lines.append("---")
            lines.append("")
        
        return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(
        description='Generate zone snapshots for historical flood events'
    )
    parser.add_argument(
        '--db-url',
        default='postgresql://flopro_admin:flopro_dev_2026@localhost/flopro_db',
        help='Database connection URL'
    )
    parser.add_argument(
        '--zones-config',
        default='zones.toml',
        help='Path to zones.toml configuration'
    )
    parser.add_argument(
        '--output',
        default='PEAK_FLOW_SUMMARY.md',
        help='Output markdown file'
    )
    
    args = parser.parse_args()
    
    print("Zone Snapshot Generator for Historical Floods")
    print("=" * 50)
    print()
    
    try:
        generator = ZoneSnapshotGenerator(args.db_url, args.zones_config)
        
        print("Connecting to database...")
        generator.connect()
        
        print("Loading zones configuration...")
        generator.load_zones_config()
        
        print("Generating zone snapshots for historical floods...")
        snapshots = generator.generate_all_snapshots()
        
        print()
        print(f"Generated {len(snapshots)} flood event snapshots")
        
        print(f"Writing report to {args.output}...")
        report = generator.format_markdown_report(snapshots)
        
        with open(args.output, 'w') as f:
            f.write(report)
        
        print()
        print(f"✓ Report saved to {args.output}")
        
    except Exception as e:
        print(f"\n✗ Error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == '__main__':
    main()
