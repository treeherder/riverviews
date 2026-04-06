# Deployment Guide — GCE + Docker Compose

This document walks through deploying the Riverviews stack on a Google Compute Engine VM using `docker compose`. The stack runs three containers: **PostgreSQL 16**, **flomon_service** (Rust daemon), and **sms_gateway** (Python → Twilio SMS).

---

## Prerequisites

| Requirement | Notes |
|-------------|-------|
| GCE VM — `e2-standard-2` or larger | 2 vCPU, 8 GB RAM recommended |
| Docker 24+ and Docker Compose v2 | Installed via the steps below |
| Google Cloud project with Pub/Sub API enabled | For SMS alert delivery |
| Twilio account | SMS sending |
| A GCP service account with `roles/pubsub.publisher` | Assigned to the VM |

---

## 1. Provision the GCE VM

```bash
gcloud compute instances create riverviews \
  --zone=us-central1-a \
  --machine-type=e2-standard-2 \
  --image-family=debian-12 \
  --image-project=debian-cloud \
  --boot-disk-size=20GB \
  --scopes=https://www.googleapis.com/auth/pubsub \
  --tags=riverviews
```

Open TCP 8080 (daemon API) — firewall rule:

```bash
gcloud compute firewall-rules create allow-riverviews-api \
  --direction=INGRESS \
  --action=ALLOW \
  --rules=tcp:8080 \
  --target-tags=riverviews \
  --source-ranges=YOUR_IP/32      # restrict to your IP
```

> Port 8081 (sms_gateway) does **not** need to be publicly accessible — Pub/Sub push
> delivers to it from within the VM network (or via an internal load balancer).

---

## 2. Install Docker on the VM

```bash
gcloud compute ssh riverviews -- -t "
  sudo apt-get update &&
  sudo apt-get install -y ca-certificates curl &&
  sudo install -m 0755 -d /etc/apt/keyrings &&
  curl -fsSL https://download.docker.com/linux/debian/gpg | sudo tee /etc/apt/keyrings/docker.asc &&
  echo \"deb [arch=amd64 signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian bookworm stable\" | sudo tee /etc/apt/sources.list.d/docker.list &&
  sudo apt-get update &&
  sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin &&
  sudo usermod -aG docker \$USER
"
```

---

## 3. Copy project files to the VM

```bash
# From your local machine:
gcloud compute scp --recurse /path/to/riverviews riverviews:~/riverviews \
  --zone=us-central1-a
```

Or clone from git if you have the repo hosted:

```bash
gcloud compute ssh riverviews -- "git clone <your-repo-url> ~/riverviews"
```

---

## 4. Configure environment variables

```bash
gcloud compute ssh riverviews -- "
  cd ~/riverviews &&
  cp .env.example .env &&
  nano .env        # fill in passwords and Twilio credentials
"
```

Set these values in `.env`:

```
POSTGRES_SUPERUSER_PASSWORD=<strong-random-password>
FLOPRO_ADMIN_PASSWORD=<strong-random-password>
TWILIO_ACCOUNT_SID=ACxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
TWILIO_AUTH_TOKEN=<your-token>
TWILIO_FROM_NUMBER=+1XXXXXXXXXX
```

---

## 5. Configure alerting

Edit `flomon_service/alerting.toml`:

```toml
[alerting]
enabled = true
pubsub_project = "your-gcp-project-id"
pubsub_topic   = "riverviews-alerts"
pubsub_enabled = true              # set true once Pub/Sub topic exists
daily_digest_hour_utc = 7

[alerting.recipients]
numbers = ["+15555550100"]         # your phone number(s)
```

---

## 6. Create the Pub/Sub topic and push subscription

```bash
# Topic (flomon_service publishes here)
gcloud pubsub topics create riverviews-alerts

# Push subscription → sms_gateway running on the VM
# Replace VM_IP with the internal or external IP of your GCE instance.
gcloud pubsub subscriptions create riverviews-sms \
  --topic=riverviews-alerts \
  --push-endpoint=http://VM_IP:8081/pubsub \
  --ack-deadline=30
```

> **Internal-only option:** If you configure the push endpoint using the VM's internal IP
> you avoid exposing port 8081 publicly. Use `gcloud compute instances describe riverviews`
> to find the internal IP.

---

## 7. Build and launch the stack

```bash
gcloud compute ssh riverviews -- "
  cd ~/riverviews &&
  docker compose build &&
  docker compose up -d
"
```

Check service health:

```bash
docker compose ps
docker compose logs -f flomon_service
curl http://localhost:8080/health
curl http://localhost:8081/health
```

---

## 8. Verify alerting

Set `pubsub_enabled = false` in `alerting.toml` first to do a dry-run — alerts are
printed to the flomon_service log instead of being published to Pub/Sub.

Once you have confirmed alerts appear in the log, set `pubsub_enabled = true` and
restart the daemon:

```bash
docker compose restart flomon_service
```

To trigger a test SMS manually:

```bash
curl -X POST http://localhost:8081/pubsub \
  -H 'Content-Type: application/json' \
  -d '{
    "message": {
      "data": "'$(echo -n '{"body":"Test alert from Riverviews","recipients":["+15555550100"],"event_time":"2026-04-05T00:00:00Z","severity":"action","site_code":"test"}' | base64 -w0)'"
    }
  }'
```

---

## 9. Keep the stack running across reboots

```bash
gcloud compute ssh riverviews -- "
  sudo crontab -l 2>/dev/null | { cat; echo '@reboot sleep 15 && cd /home/$(whoami)/riverviews && docker compose up -d >> /var/log/riverviews-start.log 2>&1'; } | sudo crontab -
"
```

Or use a systemd unit — see `docker compose` systemd integration documentation.

---

## Updating the deployment

```bash
gcloud compute ssh riverviews -- "
  cd ~/riverviews &&
  git pull &&
  docker compose build flomon_service &&
  docker compose up -d flomon_service
"
```

TOML configuration files (`alerting.toml`, `usgs_stations.toml`, etc.) are mounted as volumes — **changes take effect on daemon restart without a rebuild**:

```bash
# Edit the file locally, scp it up, then restart:
docker compose restart flomon_service
```

---

## Troubleshooting

| Symptom | Check |
|---------|-------|
| No SMS received | `docker compose logs sms_gateway` — look for Twilio errors |
| Daemon not polling | `docker compose logs flomon_service` — DB connectivity? TOML missing? |
| Pub/Sub not delivering | `gcloud pubsub subscriptions describe riverviews-sms` — check `ackDeadlineSeconds`, delivery errors |
| DB migration failed | `docker compose logs postgres` on first start — inspect init-db.sh output |
| CWMS data missing | See CWMS_INTEGRATION_SUMMARY.md — silent catalog discovery failure |
