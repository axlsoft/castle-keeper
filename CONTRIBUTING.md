# Contributing to Castle Keeper

## Prerequisites

- Node.js 24 (LTS) and npm — see `.nvmrc`
- Rust (stable) with Cargo
- The [Tauri 2 system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS
- Docker or Podman with Compose, for the local Kafka cluster

## Run the app

```bash
npm ci
npm run tauri dev
```

## Local Kafka cluster

`compose.yaml` runs a single-node Apache Kafka 4.3 broker in KRaft mode, bound to
`127.0.0.1:9092` only.

```bash
docker compose up -d --wait   # healthy in well under a minute
docker compose down           # stop it (add -v to also drop data)
```

With Podman, `podman compose` or `docker-compose` against the Podman socket work the
same way.

### Sample data

`scripts/seed.sh` fills the local broker with realistic data: `orders` (6
partitions, JSON with headers), `payments`, a compacted `customers` topic,
`audit.log`, `metrics.raw` (12 partitions) and `dlq.orders`, plus a lagging
consumer group (`billing-service`) and two caught-up, member-less groups
(`analytics`, `reporting`). It only talks to the compose broker and is safe to
re-run.

```bash
docker compose up -d --wait
scripts/seed.sh
```

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
npm run check:version
(cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
```

Tauri Rust crates and their `@tauri-apps/*` npm packages must share the same
major.minor version. Bump them together.

## CI

`.github/scripts/ci-plan.sh` decides what each run builds:

- **Pull requests** run only the checks for what changed (frontend, Rust or
  both). Rust runs clippy and tests on Linux, Windows and macOS arm64, and
  builds no installers.
- **Pushes to `main`** run everything and build installers for Linux, Windows
  and macOS arm64.
- **`v*` tags** also build the macOS x86_64 installer.
- **Manual runs** (Actions → CI → Run workflow) take a comma-separated platform
  list, e.g. `windows-x86_64`, and can build installers on request.
- A weekly scheduled run covers Linux and the Kerberos build.

Branch protection should require the **CI result** check, which passes when
every planned job passed and ignores jobs the plan skipped.

## Releasing

The app version lives in `package.json`, `package-lock.json`,
`src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`.
Don't edit it by hand. From an up-to-date `main`:

```bash
npm run release -- 0.2.0 --dry-run   # optional preview
npm run release -- 0.2.0             # bump, commit and tag v0.2.0 locally
git push --atomic origin main v0.2.0
```

Versions are `MAJOR.MINOR.PATCH`, or `MAJOR.MINOR.PATCH-N` for a pre-release,
because Windows MSI installers reject anything else.

Pushing the tag runs CI on all four platforms. CI fails straight away if the
tag doesn't match the version in those files. When every job passes, the
**Draft release** job creates a draft GitHub Release. It attaches the installers
(`.dmg` for macOS arm64 and x86_64, `.msi` and `-setup.exe` for Windows, and
`.deb`, `.rpm` and `.AppImage` for Linux), a `SHA256SUMS` file and notes
generated from merged pull requests. Review the draft on the Releases page,
edit the notes if needed, then publish it. Re-running a tag build refreshes the
files on the existing draft.

The installers are unsigned. On macOS, users have to right-click the app and
choose Open the first time. On Windows, SmartScreen warns before installing.
