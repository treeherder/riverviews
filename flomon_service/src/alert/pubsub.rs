/// Google Cloud Pub/Sub publisher (REST API, no GCP SDK).
///
/// Uses the Pub/Sub REST endpoint:
///   POST https://pubsub.googleapis.com/v1/projects/{project}/topics/{topic}:publish
///
/// Authentication uses the GCE metadata server to obtain an OAuth2 access token
/// automatically when running on a GCE instance (or Cloud Run). No service account
/// key file is required — just ensure the VM's service account has the
/// `roles/pubsub.publisher` IAM role.
///
/// When `pubsub_enabled = false` in alerting.toml the message is logged to stdout
/// and no HTTP call is made — safe for local development.

use serde::Serialize;
use std::error::Error;

/// A message payload published to Pub/Sub.
///
/// The `sms_gateway` subscriber reads `body` and forwards it to every
/// phone number listed in `recipients`.
#[derive(Debug, Serialize)]
pub struct AlertMessage {
    /// Plain-text body of the SMS to deliver.
    pub body: String,
    /// E.164 phone numbers that should receive the SMS.
    pub recipients: Vec<String>,
    /// ISO 8601 timestamp of the event that triggered this alert.
    pub event_time: String,
    /// Machine-readable severity tag (e.g. "major", "all_clear", "digest").
    pub severity: String,
    /// Site code of the triggering gauge, or "system" for daemon-level events.
    pub site_code: String,
}

// ---------------------------------------------------------------------------
// Internal Pub/Sub REST wire types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PubSubRequest {
    messages: Vec<PubSubMessage>,
}

#[derive(Serialize)]
struct PubSubMessage {
    data: String, // base64-encoded UTF-8
}

// ---------------------------------------------------------------------------
// GCE metadata token fetch
// ---------------------------------------------------------------------------

/// Fetches a short-lived OAuth2 access token from the GCE instance metadata
/// server. This only works when the process is running on GCE/Cloud Run and
/// the service account has pubsub.publisher.
fn fetch_gce_access_token(client: &reqwest::blocking::Client) -> Result<String, Box<dyn Error>> {
    let url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
    let resp = client
        .get(url)
        .header("Metadata-Flavor", "Google")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .map_err(|e| format!("GCE metadata server unreachable: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "GCE metadata token request failed: HTTP {}",
            resp.status()
        )
        .into());
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    let token: TokenResponse = resp.json()?;
    Ok(token.access_token)
}

// ---------------------------------------------------------------------------
// Publish
// ---------------------------------------------------------------------------

/// Publishes an alert message to a Pub/Sub topic.
///
/// If `pubsub_enabled` is `false` the message is printed to stdout and `Ok(())`
/// is returned — no HTTP call is made.
pub fn publish(
    client: &reqwest::blocking::Client,
    project: &str,
    topic: &str,
    message: &AlertMessage,
    pubsub_enabled: bool,
) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::to_string(message)?;

    if !pubsub_enabled {
        println!(
            "[ALERT DRY-RUN] site={} severity={} | {}",
            message.site_code, message.severity, message.body
        );
        return Ok(());
    }

    let token = fetch_gce_access_token(client)?;

    let encoded = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        payload.as_bytes(),
    );

    let request = PubSubRequest {
        messages: vec![PubSubMessage { data: encoded }],
    };

    let url = format!(
        "https://pubsub.googleapis.com/v1/projects/{}/topics/{}:publish",
        project, topic
    );

    let resp = client
        .post(&url)
        .bearer_auth(&token)
        .json(&request)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("Pub/Sub publish request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!(
            "Pub/Sub publish returned HTTP {}: {}",
            status, body
        )
        .into());
    }

    Ok(())
}
