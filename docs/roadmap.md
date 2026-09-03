# Roadmap

## 0.1 architecture

- `Cli` / `ModalCli`: typed argv-only API with no Serde dependency.
- `Config` / `ModalConfig`: layered `defaults < file < env < argv` API.
- one clap command model for parsing, help, aliases, subcommands, and completions.
- one optional kwconf derive layer; clap derive is not enabled underneath it.
- direct typed mutation from `T::default()` rather than whole-config
  serialize/merge/deserialize round trips.
- config structs themselves do not require Serde derives.
- type-directed raw parsing for full `Config` and `FromStr` parsing for
  lightweight `Cli`.
- nested subconfigs, modal commands, aliases, choices, strict bool negation,
  JSON/TOML by default, optional YAML, env bindings, help color, and optional
  shell-completion generation.
- direct `clap_builder` use; no clap derive layer.
- `serde_core` for the full-config trait surface and parse-only TOML.
- feature-matrix CI for core, derive-only, default, and all-feature builds.

## Next useful work

- snapshot tests for representative help/error output;
- record cold compile timings for core, derive-only, default, and full builds
  before optimizing metadata lookup;
- use downstream reports and timings to decide whether JSON should also become
  independently selectable;
- add short options and positionals only where they map cleanly onto the config
  object model;
- improve custom scalar parsing ergonomics for `Cli` beyond manual `FromStr` if
  real ports show a repeated need;
- install docs for completion scripts.

## Keep deferred

Do not build a general-purpose CLI builder. Clap already owns that problem.
Kwconf should concentrate on the typed configuration-object model and source
layering that distinguish it from clap.
