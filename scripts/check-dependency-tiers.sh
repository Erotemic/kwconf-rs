#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

fail_if_matches() {
    local label="$1"
    local pattern="$2"
    local tree="$3"
    if grep -Eq "$pattern" <<<"$tree"; then
        echo "$label contains a dependency that should be absent" >&2
        grep -E "$pattern" <<<"$tree" >&2 || true
        exit 1
    fi
}

core_tree="$(cargo tree -p kwconf --no-default-features -e normal)"
fail_if_matches \
    "no-default-features" \
    'kwconf_derive v|serde(_core|_json|_derive)? v|syn v|quote v|proc-macro2 v|toml v|yaml_serde v|clap_complete v' \
    "$core_tree"

derive_tree="$(cargo tree -p kwconf --no-default-features --features derive -e normal)"
fail_if_matches \
    "derive-only" \
    'serde(_core|_json|_derive)? v|toml v|yaml_serde v|clap_complete v' \
    "$derive_tree"

default_tree="$(cargo tree -p kwconf -e normal)"
fail_if_matches \
    "default features" \
    'serde_derive v|yaml_serde v|clap_complete v|toml_writer v|(^|[^[:alnum:]_])clap v[0-9]' \
    "$default_tree"

printf '%s\n' "dependency tiers look correct"
