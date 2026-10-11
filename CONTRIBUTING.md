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

### SASL broker

The `sasl` compose profile adds a second broker on `127.0.0.1:9094` that accepts
SASL/PLAIN, SCRAM-SHA-256 and SCRAM-SHA-512 (`SASL_PLAINTEXT`). The test user is
`alice` / `alice-secret`. These credentials are for localhost only.

```bash
docker compose --profile sasl up -d --wait
```

### Integration tests

`src-tauri/tests/` covers topic listing, produce round trips, broker-down
timeouts, the write guard and SASL/SCRAM auth. Tests that need a broker pass
without running unless their address is set, so plain `cargo test` works
without Docker. To run everything against the compose brokers:

```bash
npm run test:integration    # starts plaintext + SASL brokers, then cargo test
```

or by hand:

```bash
cd src-tauri
CK_IT_BOOTSTRAP=127.0.0.1:9092 CK_IT_SASL_BOOTSTRAP=127.0.0.1:9094 cargo test -- --test-threads=1
```

## Native dependencies

The Kafka client (librdkafka) is compiled into the app together with OpenSSL
and zlib, so release builds depend only on libraries the OS provides. SASL/PLAIN
and SCRAM are built into librdkafka.

Kerberos (SASL/GSSAPI) is behind the off-by-default `kerberos` cargo feature. It
links the system Cyrus SASL and krb5 libraries (Linux: `libsasl2-dev`,
`libkrb5-dev`) and is Unix-only. Kerberos on Windows (SSPI) is not supported.

```bash
cd src-tauri && cargo build --features kerberos
```

On Windows, librdkafka is built with CMake, so CMake must be on `PATH` (it is on
GitHub's Windows runners and in Visual Studio's C++ workload).

## Checks

Run these before committing. CI runs the same set.

```bash
npm run check
npm test
npm run build
npm run check:tauri-versions
(cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
```

Tauri Rust crates and their `@tauri-apps/*` npm packages must share the same
major.minor version. Bump them together.
