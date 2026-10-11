# Contributing to Castle Keeper

## Prerequisites

- Node.js 22+ and npm
- Rust (stable) with Cargo
- The [Tauri 2 system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS
- Docker or Podman with Compose, for the local Kafka cluster

## Run the app

```bash
npm ci
npm run tauri dev
```

## Local Kafka cluster

`compose.yaml` runs a single-node Apache Kafka 4.1 broker in KRaft mode, bound to
`127.0.0.1:9092` only.

```bash
docker compose up -d --wait   # healthy in well under a minute
docker compose down           # stop it (add -v to also drop data)
```

With Podman, `podman compose` or `docker-compose` against the Podman socket work the
same way.

List topics from inside the container:

```bash
docker compose exec kafka /opt/kafka/bin/kafka-topics.sh --bootstrap-server localhost:9092 --list
```

## Checks

Run these before committing. CI runs the same set.

```bash
npm run check
npm run build
npm run check:tauri-versions
(cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
```

Tauri Rust crates and their `@tauri-apps/*` npm packages must share the same
major.minor version. Bump them together.
