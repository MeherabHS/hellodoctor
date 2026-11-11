#!/usr/bin/env bash
# Repeatable local E2E for FINANCIAL_MODULE_MODE=deferred (see e2e_deferred_booking.py).
# Provisions a DISPOSABLE PostgreSQL 16 + Redis (docker), generates throwaway secrets in memory
# (nothing is written to disk or committed), boots the real backend binary, runs the checks, tears down.
# Requires: docker, cargo, openssl, python3. Ports: PG 55433, Redis 16381, API 8090 (override below).
set -euo pipefail
cd "$(dirname "$0")/../.."          # services/backend
PG=${E2E_PG_CONTAINER:-hd-e2e-pg}; RD=${E2E_REDIS_CONTAINER:-hd-e2e-redis}
PG_PORT=${E2E_PG_PORT:-55433}; RD_PORT=${E2E_REDIS_PORT:-16381}; API_PORT=${E2E_API_PORT:-8090}
PW=$(openssl rand -hex 16); TMP=$(mktemp -d); SERVER_PID=""
cleanup() { [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true; docker rm -f "$PG" "$RD" >/dev/null 2>&1 || true; rm -rf "$TMP"; }
trap cleanup EXIT
docker rm -f "$PG" "$RD" >/dev/null 2>&1 || true
docker run -d --name "$PG" -e POSTGRES_USER=hellodoctor -e POSTGRES_PASSWORD="$PW" -e POSTGRES_DB=hellodoctor -p 127.0.0.1:$PG_PORT:5432 postgres:16 >/dev/null
docker run -d --name "$RD" -p 127.0.0.1:$RD_PORT:6379 redis:7-alpine >/dev/null
until docker exec "$PG" pg_isready -U hellodoctor -d hellodoctor -q; do sleep 1; done; sleep 3
openssl genpkey -algorithm ed25519 -outform DER -out "$TMP/priv.der" 2>/dev/null
openssl pkey -in "$TMP/priv.der" -inform DER -pubout -outform DER -out "$TMP/pub.der" 2>/dev/null
export ENVIRONMENT=development HOST=127.0.0.1 PORT=$API_PORT RUST_LOG=hellodoctor_backend=info
export SEED_DEV_DATA=true DEV_AUTH_OTP_ENABLED=true FINANCIAL_MODULE_MODE=deferred
export DATABASE_URL=postgres://hellodoctor:$PW@127.0.0.1:$PG_PORT/hellodoctor
export APP_DB_HOST=127.0.0.1 APP_DB_PORT=$PG_PORT APP_DB_NAME=hellodoctor REDIS_URL=redis://127.0.0.1:$RD_PORT/5
export JWT_ED25519_PRIVATE_KEY_B64=$(base64 -w0 "$TMP/priv.der") JWT_ED25519_PUBLIC_KEY_B64=$(base64 -w0 "$TMP/pub.der")
export TOTP_ENCRYPTION_KEY_B64=$(openssl rand -base64 32) AGORA_APP_ID=$(openssl rand -hex 16) AGORA_APP_CERTIFICATE=$(openssl rand -hex 16)
export WEBHOOK_HMAC_SECRET=$(openssl rand -hex 32) WEB_AUTH_ASSERTION_SECRET=$(openssl rand -hex 24) ALLOWED_ORIGINS=http://localhost:3000
for k in API AUTH_WORKER PAYMENT_WORKER SYSTEM_WORKER FINANCE_WORKER; do export DB_${k}_ROLE_PASSWORD=$(openssl rand -hex 16); done
cargo build --locked --bin hellodoctor-backend
./target/debug/hellodoctor-backend >"$TMP/server.log" 2>&1 & SERVER_PID=$!
for _ in $(seq 1 90); do curl -s -o /dev/null "http://127.0.0.1:$API_PORT/api/v1/config/booking-policy" && break; sleep 2; done
E2E_API_BASE="http://127.0.0.1:$API_PORT/api/v1/" E2E_PG_CONTAINER="$PG" "${PYTHON:-$(command -v python3 || command -v python)}" scripts/local_e2e/e2e_deferred_booking.py
