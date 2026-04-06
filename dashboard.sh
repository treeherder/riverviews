#!/usr/bin/env bash
# Launch the Riverviews flood monitoring dashboard.
# Activates the floml venv automatically.
#
# Usage:
#   ./dashboard.sh                          # connect to localhost:8080
#   ./dashboard.sh --api http://HOST:8080   # SSH tunnel or remote host

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV="$SCRIPT_DIR/floml/venv"
DASHBOARD="$SCRIPT_DIR/floml/scripts/zone_dashboard.py"

if [[ ! -d "$VENV" ]]; then
    echo "venv not found at $VENV"
    echo "Run:  cd floml && python3 -m venv venv && source venv/bin/activate && pip install -r requirements.txt"
    exit 1
fi

source "$VENV/bin/activate"
exec python3 "$DASHBOARD" "$@"
