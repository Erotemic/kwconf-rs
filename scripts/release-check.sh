#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy -p kwconf --lib --no-default-features -- -D warnings
cargo clippy -p kwconf --example cli --no-default-features --features derive -- -D warnings

cargo check -p kwconf --lib --no-default-features
cargo check -p kwconf --lib --no-default-features --features derive
cargo check -p kwconf --example cli --no-default-features --features derive
cargo check -p kwconf --lib --no-default-features --features completion
cargo test -p kwconf --lib --no-default-features --features config
cargo test -p kwconf --lib --no-default-features --features config,toml
cargo test -p kwconf --lib --no-default-features --features config,yaml
cargo test --workspace
cargo test --workspace --all-features

scripts/check-dependency-tiers.sh

RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

for crate in kwconf_derive kwconf; do
    listing="$(cargo package -p "$crate" --list)"
    printf '%s\n' "$listing"
    grep -qx 'LICENSE-APACHE' <<<"$listing"
    grep -qx 'LICENSE-MIT' <<<"$listing"
done

cargo publish --workspace --dry-run
