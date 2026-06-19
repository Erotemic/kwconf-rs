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

## Next useful work

- better error provenance;
- snapshot tests for help text;
- install docs for completion scripts;
- a small migration guide for real kwconf CLIs;
- inline modal fields if real ports need them;
- deeper clap interop.

## Keep deferred

Avoid re-implementing all of `clap`. Use the Rust ecosystem where it is already
strong, and keep kwconf-rs focused on the config contract.
