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
  TOML/JSON/YAML, env bindings, help color, and completions.
- feature-matrix CI for no-default-features and derive-only builds.

## Next useful work

- snapshot tests for representative help/error output;
- benchmark cold compile and CLI startup before optimizing metadata lookup;
- decide whether config formats should become individually selectable features
  (`toml`, `json`, `yaml`) after real consumers establish which combinations
  are useful;
- add short options and positionals only where they map cleanly onto the config
  object model;
- improve custom scalar parsing ergonomics for `Cli` beyond manual `FromStr` if
  real ports show a repeated need;
- install docs for completion scripts.

## Keep deferred

Do not build a general-purpose CLI builder. Clap already owns that problem.
Kwconf should concentrate on the typed configuration-object model and source
layering that distinguish it from clap.
