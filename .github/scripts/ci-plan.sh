#!/usr/bin/env bash
# Decides which CI jobs run, and on which platforms, for this event.
#
#   pull_request       Frontend and Rust checks only for the parts that
#                      changed. Rust: Linux, Windows and macOS arm64 run
#                      clippy + tests; no release builds or bundles.
#   push to main       Everything, plus release bundles for Linux, Windows
#                      and macOS arm64.
#   push of a v* tag   Everything, plus bundles for all four platforms
#                      (adds macOS x86_64).
#   workflow_dispatch  The platforms chosen in the run form; bundles if
#                      asked for.
#   schedule           Weekly Linux run, including the Kerberos build.
#
# Inputs (environment): EVENT, REF, BASE_SHA (pull requests; HEAD_SHA
# defaults to HEAD), PLATFORMS and BUNDLES (manual runs). Writes
# frontend/rust/kerberos/matrix to $GITHUB_OUTPUT, or to stdout locally.
set -euo pipefail

out="${GITHUB_OUTPUT:-/dev/stdout}"
EVENT="${EVENT:?}"
REF="${REF:-}"

# name|runner|target
PLATFORM_TABLE="linux-x86_64|ubuntu-latest|x86_64-unknown-linux-gnu
windows-x86_64|windows-latest|x86_64-pc-windows-msvc
macos-arm64|macos-latest|aarch64-apple-darwin
macos-x86_64|macos-15-intel|x86_64-apple-darwin"
ALL_PLATFORMS="linux-x86_64 windows-x86_64 macos-arm64 macos-x86_64"

frontend=true
rust=true
kerberos=true

if [[ "$EVENT" == "pull_request" ]]; then
    files="$(git diff --name-only "${BASE_SHA:?}" "${HEAD_SHA:-HEAD}")"
    echo "Changed files:" >&2
    echo "$files" | sed 's/^/  /' >&2
    changed() { grep -qE "$1" <<<"$files"; }

    # Changes to CI itself always run everything it controls.
    ci='^(\.github/workflows/ci\.yml|\.github/scripts/)'
    frontend=false
    rust=false
    kerberos=false
    changed "$ci|^(src/|static/|package(-lock)?\.json$|vite\.config\.js$|tsconfig\.json$|components\.json$|\.nvmrc$|scripts/[^/]+\.mjs$|src-tauri/(Cargo\.(toml|lock)|tauri\.conf\.json)$)" && frontend=true
    # The bindings test compares src/lib/bindings.ts with the Rust types.
    changed "$ci|^(src-tauri/|src/lib/bindings\.ts$|compose\.yaml$|dev/|scripts/(seed|test-integration)\.sh$)" && rust=true
    changed "$ci|^src-tauri/Cargo\.(toml|lock)$" && kerberos=true
fi

bundle=false
case "$EVENT" in
    pull_request)
        platforms="linux-x86_64 windows-x86_64 macos-arm64" ;;
    push)
        bundle=true
        if [[ "$REF" == refs/tags/* ]]; then
            platforms="$ALL_PLATFORMS"
        else
            platforms="linux-x86_64 windows-x86_64 macos-arm64"
        fi ;;
    workflow_dispatch)
        requested="${PLATFORMS:-all}"
        [[ "$requested" == "all" ]] && requested="$ALL_PLATFORMS"
        platforms="$(tr ', ' '\n\n' <<<"$requested" | grep -v '^$' | tr '\n' ' ')"
        [[ "${BUNDLES:-false}" == "true" ]] && bundle=true ;;
    schedule)
        platforms="linux-x86_64" ;;
    *)
        echo "Unhandled event: $EVENT" >&2
        exit 1 ;;
esac

entries=()
for name in $platforms; do
    row="$(grep "^${name}|" <<<"$PLATFORM_TABLE" || true)"
    if [[ -z "$row" ]]; then
        echo "Unknown platform '$name'. Choose from: $ALL_PLATFORMS (or all)." >&2
        exit 1
    fi
    IFS='|' read -r _ runner target <<<"$row"
    entries+=("$(jq -cn --arg name "$name" --arg os "$runner" --arg target "$target" --argjson bundle "$bundle" \
        '{name: $name, os: $os, target: $target, bundle: $bundle}')")
done
matrix="$(printf '%s\n' "${entries[@]}" | jq -cs '{include: .}')"

{
    echo "frontend=$frontend"
    echo "rust=$rust"
    echo "kerberos=$kerberos"
    echo "matrix=$matrix"
} >>"$out"

echo "Plan: frontend=$frontend rust=$rust kerberos=$kerberos" >&2
echo "Matrix: $matrix" >&2
