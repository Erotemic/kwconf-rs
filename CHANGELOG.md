# Changelog

All notable changes to `kwconf` and `kwconf_derive` are recorded here. Both
crates share a version number.

## 0.1.0 (unreleased)

First release.

### Architecture

- Split the public model into lightweight `Cli` / `ModalCli` and layered
  `Config` / `ModalConfig`.
- `Cli` needs no Serde dependency. `kwconf --no-default-features --features
  derive` provides the Python-like struct API with one kwconf proc-macro layer.
- `kwconf --no-default-features` has neither Serde nor a proc-macro dependency.
- Full `Config` no longer serializes defaults and deserializes the whole struct.
  Resolution starts from `T::default()` and applies winning values through
  derive-generated typed field setters, so config structs themselves no longer
  require Serde derives.
- One clap command model owns argv recognition, help, aliases, subcommands, and
  completions. Clap's derive feature is not enabled.
- Removed the unnecessary Syn `extra-traits` feature.

### Behavior

- Full config precedence is `defaults < config file < env < argv`.
- `Config::from_iter` is argv-only; `Config::try_cli` additionally reads the
  declared process-environment bindings.
- String-like raw values are decoded by destination type, so `"123"` stays a
  `String` but becomes `123` for an integer field.
- Bool text accepts `true/false` and `1/0`; `yes/no` and `on/off` are rejected.
- CSV parsing preserves explicit empty components. `a,,b,` has four fields and
  an empty CSV string is one empty string field. This intentionally does not
  copy Python kwconf's empty-component filtering.
- YAML parse errors remain errors for `String` and enum destinations instead of
  falling back to the raw token.
- Only declared env bindings are read from the process, preventing unrelated
  non-Unicode environment entries from panicking parsing.
- Config paths remain `PathBuf`s, including non-UTF-8 paths where supported.
- Unknown config-file extensions report TOML, JSON, and YAML parse failures
  together.

### API

- Added `#[derive(kwconf::Cli)]` and `#[derive(kwconf::ModalCli)]`.
- `Config` no longer requires `Serialize + DeserializeOwned` on the outer
  config type.
- Renamed the generic conversion error from `Error::Deserialize` to
  `Error::InvalidValue`.
- Doc comments become help/about text when explicit `help` / `about` metadata
  is absent.
- Derive metadata remains under `#[doc(hidden)] kwconf::__private`.
- `Error` is `#[non_exhaustive]` and exposes the underlying I/O source.
- Generic config structs and `#[kwconf(crate = "path")]` remain supported.
- Minimum supported Rust version is 1.85.
