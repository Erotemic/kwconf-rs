# Release checks

`kwconf` treats downstream build cost as part of the release contract. The
normal default build should provide the layered configuration experience without
pulling optional YAML or completion-generation dependencies into every consumer.

## One-command release gate

From the repository root:

```bash
scripts/release-check.sh
```

This checks formatting, clippy, minimal/derive/config feature combinations,
default and all-feature tests, dependency-tier invariants, rustdoc, package
contents, and a workspace publish dry run.

CI repeats the same classes of checks and also builds/tests all features on the
Rust 1.85 MSRV.

## Measure cold downstream build cost

Run:

```bash
scripts/build-cost.sh
```

The script measures four library tiers with independent target directories:

```text
core          no default features

derive-cli    derive only, no Serde

default       derive + config + JSON + TOML

full          every feature
```

It does not run `cargo clean` and does not invalidate the repository's ordinary
`target` directory. Each invocation writes fresh Cargo timing reports under
`target/kwconf-build-cost/...` and prints the dependency trees afterward.

For build-cost decisions, use these `cargo build` measurements rather than
`cargo test`: kwconf's test suite intentionally has dev-dependencies such as
`trybuild` and Serde derives that downstream library users do not inherit.

## Package inspection

Both publishable crates carry their MIT and Apache-2.0 license texts inside the
crate package. The release gate asserts that both files appear in
`cargo package --list` before the publish dry run.

When dependency versions change, let Cargo refresh `Cargo.lock` on a
Rust-capable machine and include the resulting lockfile update in the release
commit.
