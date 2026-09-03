# Changelog

All notable changes to `kwconf` and `kwconf_derive` are recorded here. Both
crates share a version number.

## 0.1.0 (unreleased)

First release.

### Behavior

- Values resolve as `defaults < config file < env < argv`.
- argv and env text is coerced by the destination field type instead of by
  its spelling: `"123"` stays a string for a `String` field and becomes an
  integer for a `u32` field. `bool` accepts `true/false`, `yes/no`, `on/off`,
  and `1/0`. `csv` coerces each element by the element type.
- One `clap` command model owns argv recognition, help, and completion
  scripts. `--no-flag` is a real flag, aliases apply to every component of a
  dotted path, dashes and underscores are interchangeable, and anything after
  `--` is an error instead of being dropped.
- Only declared env bindings are read from the process, so an unrelated
  non-Unicode environment variable cannot panic a CLI. `--config` accepts
  non-UTF-8 paths.
- A subcommand `--config` file layers over the root file's variant table.
  Modal table keys are matched dash/underscore-insensitively.
- Schema collisions (name vs alias, dash/underscore twins, `no_x` vs the
  negation of bool `x`, modal variant aliases, user fields named after an
  enabled special option, and the reserved `help`) are compile errors within
  one struct and `Error::Schema` across nested subconfigs.
- Config files with an unknown extension report the TOML, JSON, and YAML
  parse errors together.

### API

- Public surface: `Config`, `ModalConfig`, `Sources`, `Error`, `Help`,
  `Result`, `ColorChoice`, `CompletionShell`, and the two derives. Derive
  metadata lives under `#[doc(hidden)] kwconf::__private`.
- `Error` is `#[non_exhaustive]`, implements `source()` for `Io`, and reports
  deserialization failures as `Deserialize { field, message }` with the dotted
  field path. `HelpRequested` carries a `Help` value with plain and ANSI text.
- `Sources` fields are private; use the builder methods.
  `Sources::with_process_env(bool)` controls whether declared env bindings are
  read from the process. `Sources::from_iter` and `Config::from_iter` still
  read the process environment; `Sources::empty()` does not.
- Generic config structs are supported. `#[kwconf(crate = "path")]` points the
  derive at a renamed dependency.
- Minimum supported Rust version is 1.85.
