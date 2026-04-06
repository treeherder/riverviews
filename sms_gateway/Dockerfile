FROM python:3.12-slim

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY app.py .

EXPOSE 8081

# TWILIO_* env vars are injected at runtime by docker-compose / GCE.
CMD ["gunicorn", "--bind", "0.0.0.0:8081", "--workers", "2", "--timeout", "30", "app:app"]
