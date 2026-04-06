"""
sms_gateway — Google Cloud Pub/Sub → Twilio SMS bridge.

Runs as a Cloud Run or docker-compose service.  It exposes a single
HTTP endpoint that accepts Pub/Sub push messages and forwards the alert
body as an SMS to every recipient listed in the message payload.

Environment variables (required):
    TWILIO_ACCOUNT_SID   Twilio account SID
    TWILIO_AUTH_TOKEN    Twilio auth token
    TWILIO_FROM_NUMBER   E.164 number to send from (e.g. +15555550199)

Optional:
    PORT                 HTTP listen port (default 8081)
    LOG_LEVEL            Python logging level (default INFO)

Pub/Sub push subscription should POST to http://<host>:<PORT>/pubsub.

Message payload (JSON, base64-encoded in the Pub/Sub wrapper):
    {
        "body":       "FLOOD at Kingston Mines: 14.30 ft ...",
        "recipients": ["+15555550100", "+15555550101"],
        "event_time": "2026-04-05T14:00:00Z",
        "severity":   "flood",
        "site_code":  "05568500"
    }
"""

import base64
import json
import logging
import os

from dotenv import load_dotenv
from flask import Flask, jsonify, request
from twilio.rest import Client as TwilioClient

load_dotenv()

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

logging.basicConfig(
    level=os.environ.get("LOG_LEVEL", "INFO"),
    format="%(asctime)s [%(levelname)s] %(message)s",
)
log = logging.getLogger(__name__)

TWILIO_ACCOUNT_SID = os.environ["TWILIO_ACCOUNT_SID"]
TWILIO_AUTH_TOKEN = os.environ["TWILIO_AUTH_TOKEN"]
TWILIO_FROM_NUMBER = os.environ["TWILIO_FROM_NUMBER"]
PORT = int(os.environ.get("PORT", "8081"))

twilio = TwilioClient(TWILIO_ACCOUNT_SID, TWILIO_AUTH_TOKEN)

app = Flask(__name__)

# ---------------------------------------------------------------------------
# Health check
# ---------------------------------------------------------------------------

@app.get("/health")
def health():
    return jsonify({"status": "ok"}), 200

# ---------------------------------------------------------------------------
# Pub/Sub push endpoint
# ---------------------------------------------------------------------------

@app.post("/pubsub")
def receive_pubsub():
    """Accept a Pub/Sub push message and dispatch SMS."""
    envelope = request.get_json(silent=True)
    if not envelope or "message" not in envelope:
        log.warning("Received malformed Pub/Sub envelope")
        # Return 204 so Pub/Sub does not retry undeliverable payloads.
        return "", 204

    raw = envelope["message"].get("data", "")
    try:
        payload_bytes = base64.b64decode(raw)
        payload = json.loads(payload_bytes)
    except Exception as exc:
        log.error("Failed to decode Pub/Sub message: %s", exc)
        return "", 204

    body = payload.get("body", "")
    recipients = payload.get("recipients", [])
    severity = payload.get("severity", "unknown")
    site_code = payload.get("site_code", "unknown")

    if not body or not recipients:
        log.warning("Message missing body or recipients — skipping (site=%s)", site_code)
        return "", 204

    log.info("Dispatching %s alert for site %s to %d recipient(s)", severity, site_code, len(recipients))

    errors = []
    for number in recipients:
        try:
            msg = twilio.messages.create(
                body=body,
                from_=TWILIO_FROM_NUMBER,
                to=number,
            )
            log.info("  SMS sent to %s — SID %s", number, msg.sid)
        except Exception as exc:
            log.error("  Failed to send SMS to %s: %s", number, exc)
            errors.append(str(exc))

    if errors:
        # Return 500 only if ALL sends failed — partial success is still a 200
        # so the message is acked from Pub/Sub.
        if len(errors) == len(recipients):
            return jsonify({"errors": errors}), 500

    return "", 204

# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    log.info("sms_gateway starting on port %d", PORT)
    app.run(host="0.0.0.0", port=PORT)
