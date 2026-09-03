#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Each tier gets a new target directory. This measures a genuinely cold build
# without deleting or invalidating the developer's ordinary target directory.
root="${KWCONF_TIMING_ROOT:-target/kwconf-build-cost/$(date +%Y%m%d-%H%M%S)-$$}"
mkdir -p "$root"

measure() {
    local name="$1"
    shift
    local target="$root/$name"
    echo
    echo "== $name =="
    echo "target: $target"
    CARGO_TARGET_DIR="$target" cargo build "$@" --timings
    echo "timing report: $target/cargo-timings/cargo-timing.html"
}

measure core -p kwconf --lib --no-default-features
measure derive-cli -p kwconf --lib --no-default-features --features derive
measure default -p kwconf --lib
measure full -p kwconf --lib --all-features

echo
echo "== dependency trees =="
cargo tree -p kwconf --no-default-features -e normal
cargo tree -p kwconf --no-default-features --features derive -e normal
cargo tree -p kwconf -e normal
cargo tree -p kwconf --all-features -e normal
cargo tree -p kwconf -d

echo
echo "Cold-build reports are under: $root"
