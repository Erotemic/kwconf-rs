# Roadmap

## Done in the starter

- `#[derive(kwconf::Config)]`
- defaults, choices, aliases, and env bindings
- source order: `defaults < config file < env < argv`
- parser names: `auto`, `csv`, `yaml`
- TOML / JSON / YAML config files
- generated help
- `clap` color policy for help
- generated shell completion scripts
- Python kwconf / Rust kwconf-rs parity demo

## Next useful work

- better error provenance;
- nested config examples;
- modal / subcommand config;
- snapshot tests for help text;
- install docs for completion scripts;
- a small migration guide for real kwconf CLIs.

## Keep deferred

Avoid re-implementing all of `clap`. Use the Rust ecosystem where it is already
strong, and keep kwconf-rs focused on the config contract.
