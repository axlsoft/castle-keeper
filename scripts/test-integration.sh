#!/usr/bin/env bash
# Starts the local brokers (plaintext + SASL) and runs the Rust test suite
# against them. The brokers are left running; stop them with
# `docker compose --profile sasl down`.
set -euo pipefail

cd "$(dirname "$0")/.."

compose() {
  if docker compose version >/dev/null 2>&1; then docker compose "$@"; else docker-compose "$@"; fi
}

compose --profile sasl up -d --wait

# The SCRAM user is created by a one-shot job after the SASL broker is up.
for _ in $(seq 1 30); do
  if compose --profile sasl ps -a kafka-sasl-users --format '{{.State}}' 2>/dev/null | grep -q exited; then
    break
  fi
  sleep 1
done

cd src-tauri
CK_IT_BOOTSTRAP=127.0.0.1:9092 CK_IT_SASL_BOOTSTRAP=127.0.0.1:9094 \
  cargo test "$@" -- --test-threads=1
