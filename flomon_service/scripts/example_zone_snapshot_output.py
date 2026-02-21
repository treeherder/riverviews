#!/usr/bin/env python3
"""
Example demonstrating zone snapshot analysis output structure.

This shows what the generate_flood_zone_snapshots.py script produces
when analyzing historical floods with the zone-based framework.
"""

# Example zone snapshot for 2013-04-21 Kingston Mines flood (24.62 ft)
EXAMPLE_2013_COMPOUND_EVENT = {
    'event': {
        'site_code': '05568500',
        'crest_time': '2013-04-21T12:00:00+00:00',
        'peak_stage_ft': 24.62,
        'severity': 'MAJOR'
    },
    
    'event_classification': 'COMPOUND',
    'backwater_active': True,
    'upstream_pulse_detected': True,
    'local_tributary_active': True,
    
    'zones': {
        0: {  # Mississippi River - Backwater Source
            'status': 'CRITICAL',
            'sensors': {
                '05587450': {'value': 24.5, 'unit': 'ft', 'type': 'stage'},  # Grafton
                '05587500': {'value': 23.1, 'unit': 'ft', 'type': 'stage'},  # Alton
            },
            'lead_time_hours': [12, 120]
        },
        
        1: {  # LaGrange Lock - Backwater Interface
            'status': 'CRITICAL',
            'sensors': {
                'IL08P': {'value': 448.5, 'unit': 'ft', 'type': 'pool_elevation'},
                'IL08TW': {'value': 447.9, 'unit': 'ft', 'type': 'tailwater'},
                'differential': 0.6,  # < 1 ft = severe backwater
            },
            'lead_time_hours': [6, 24]
        },
        
        2: {  # Upper Peoria Lake - PROPERTY ZONE
            'status': 'CRITICAL',
            'sensors': {
                '05568500': {'value': 24.62, 'unit': 'ft', 'type': 'stage'},  # Kingston Mines
                'IL07P': {'value': 18.79, 'unit': 'ft', 'type': 'pool_elevation'},  # Peoria pool
                'KPIA': {'value': 0.8, 'unit': 'in', 'type': 'precipitation'},
            },
            'lead_time_hours': [0, 6]
        },
        
        3: {  # Local Tributaries
            'status': 'WARNING',
            'sensors': {
                '05568580': {'value': 12.5, 'unit': 'ft', 'type': 'stage'},  # Mackinaw River
                '05570910': {'value': 35.8, 'unit': 'ft', 'type': 'stage'},  # Spoon River (RECORD)
                'KBMI': {'value': 1.2, 'unit': 'in', 'type': 'precipitation'},
            },
            'lead_time_hours': [6, 18]
        },
        
        4: {  # Mid Illinois
            'status': 'WARNING',
            'sensors': {
                '05557000': {'value': 15.2, 'unit': 'ft', 'type': 'stage'},  # Henry
                '05558300': {'value': 14.8, 'unit': 'ft', 'type': 'stage'},  # Starved Rock
            },
            'lead_time_hours': [18, 48]
        },
        
        5: {  # Upper Illinois
            'status': 'ELEVATED',
            'sensors': {
                '05553700': {'value': 18000, 'unit': 'cfs', 'type': 'discharge'},  # Dresden
                '05527800': {'value': 8500, 'unit': 'cfs', 'type': 'discharge'},  # Kankakee
            },
            'lead_time_hours': [36, 72]
        },
        
        6: {  # Chicago CAWS
            'status': 'WARNING',
            'sensors': {
                '05536890': {'value': 21400, 'unit': 'cfs', 'type': 'discharge'},  # Lockport
                'KORD': {'value': 1.5, 'unit': 'in', 'type': 'precipitation_6hr'},
            },
            'lead_time_hours': [72, 120]
        }
    },
    
    'analysis': {
        'compound_mechanism': (
            'Property zone trapped between Mississippi backwater (south) '
            'and Chicago runoff (north). LaGrange differential 0.6 ft indicates '
            'severe backwater blocking southward drainage while upper basin '
            'floods continue arriving from north. This is the 2013 record flood signature.'
        ),
        
        'precursor_indicators_72hr_before': {
            'zone_6_chicago_precip_spike': True,
            'zone_5_dresden_elevated': True,
            'zone_4_henry_rising': True,
            'zone_0_grafton_above_20ft': True,
            'zone_1_differential_dropping': True,
        },
        
        'regression_features': {
            # Features 72 hours before crest
            't_minus_72hr': {
                'grafton_stage': 22.1,
                'lagrange_differential': 1.8,
                'chicago_6hr_precip': 1.5,
                'dresden_discharge': 15000,
                'upper_basin_precip_total': 3.2,
            },
            
            # Features 48 hours before crest
            't_minus_48hr': {
                'grafton_stage': 23.5,
                'lagrange_differential': 1.2,
                'henry_stage': 14.2,
                'starved_rock_stage': 13.8,
            },
            
            # Features 24 hours before crest
            't_minus_24hr': {
                'grafton_stage': 24.2,
                'lagrange_differential': 0.8,
                'kingston_stage': 22.1,
                'peoria_pool': 18.2,
                'mackinaw_rate_of_rise': 0.8,  # ft/hr
            },
            
            # Target (0 hours - at crest)
            'target': {
                'kingston_peak_stage': 24.62,
                'event_severity': 'MAJOR',
                'event_type': 'COMPOUND'
            }
        }
    }
}

# Example contrast: 1982-12-04 pure backwater event (20.21 ft)
EXAMPLE_1982_BACKWATER_EVENT = {
    'event': {
        'site_code': '05567500',
        'crest_time': '1982-12-04T12:00:00+00:00',
        'peak_stage_ft': 20.21,
        'severity': 'MODERATE'
    },
    
    'event_classification': 'BOTTOM_UP',
    'backwater_active': True,
    'upstream_pulse_detected': False,  # <- KEY DIFFERENCE
    'local_tributary_active': False,
    
    'zones': {
        0: {'status': 'CRITICAL'},  # Mississippi extreme
        1: {'status': 'CRITICAL'},  # LaGrange blocked
        2: {'status': 'CRITICAL'},  # Property flooded
        3: {'status': 'NORMAL'},    # Tributaries quiet
        4: {'status': 'ELEVATED'},  # Mid Illinois moderate
        5: {'status': 'NORMAL'},    # Upper Illinois normal <- KEY
        6: {'status': 'NORMAL'},    # Chicago normal <- KEY
    },
    
    'analysis': {
        'backwater_mechanism': (
            'Classic backwater-only event. Property at RECORD 20.21 ft while '
            'upper basin (Zones 5-6) relatively quiet. Mississippi River blocked '
            'Illinois River outflow through Alton. Counterintuitive signature: '
            'Zone 2 flooding while Zones 5-6 normal.'
        )
    }
}


def print_zone_comparison():
    """Print comparison of compound vs. backwater events"""
    
    print("ZONE-BASED EVENT COMPARISON")
    print("=" * 80)
    print()
    
    print("2013-04-21 COMPOUND EVENT (24.62 ft)")
    print("-" * 40)
    for zone_id, zone in EXAMPLE_2013_COMPOUND_EVENT['zones'].items():
        print(f"  Zone {zone_id}: {zone['status']}")
    print(f"  Event Type: {EXAMPLE_2013_COMPOUND_EVENT['event_classification']}")
    print(f"  Backwater: {EXAMPLE_2013_COMPOUND_EVENT['backwater_active']}")
    print(f"  Upstream: {EXAMPLE_2013_COMPOUND_EVENT['upstream_pulse_detected']}")
    print()
    
    print("1982-12-04 BACKWATER EVENT (20.21 ft)")
    print("-" * 40)
    for zone_id, zone in EXAMPLE_1982_BACKWATER_EVENT['zones'].items():
        print(f"  Zone {zone_id}: {zone['status']}")
    print(f"  Event Type: {EXAMPLE_1982_BACKWATER_EVENT['event_classification']}")
    print(f"  Backwater: {EXAMPLE_1982_BACKWATER_EVENT['backwater_active']}")
    print(f"  Upstream: {EXAMPLE_1982_BACKWATER_EVENT['upstream_pulse_detected']}")
    print()
    
    print("KEY INSIGHT:")
    print("  2013: ALL zones active (compound) → 24.62 ft")
    print("  1982: ONLY zones 0-2 active (backwater) → 20.21 ft")
    print("  Difference: +4.4 ft higher when upstream also flooding")
    print()


if __name__ == '__main__':
    print_zone_comparison()
