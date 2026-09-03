# Roadmap

## Done in the starter

- `#[derive(kwconf::Config)]`
- `#[derive(kwconf::ModalConfig)]`
- defaults, choices, aliases, and env bindings
- nested subconfigs with `#[kwconf(subconfig)]`
- modal subcommands with enum variants
- source order: `defaults < config file < env < argv`
- parser names: `auto`, `csv`, `yaml`
- TOML / JSON / YAML config files
- generated help
- `clap` color policy for help
- generated shell completion scripts
- Python kwconf / Rust kwconf-rs parity demo
- type-directed coercion of argv/env text (a `String` field keeps `"123"`)
- one `clap` model for parsing, help, and completions
- schema collision checks at compile time and at first use
- generic config structs and `#[kwconf(crate = ...)]`

## Next useful work

- snapshot tests for help text;
- install docs for completion scripts;
- a small migration guide for real kwconf CLIs;
- inline modal fields if real ports need them;
- deeper clap interop (short options, positionals);
- benchmarks for startup cost before adding lookup caches.

## Keep deferred

Avoid re-implementing all of `clap`. Use the Rust ecosystem where it is already
strong, and keep kwconf-rs focused on the config contract.
