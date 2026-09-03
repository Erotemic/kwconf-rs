# kwconf-rs contract

## Two layers

`kwconf-rs` has a lightweight CLI layer and a layered configuration layer.

### `Cli`

`#[derive(kwconf::Cli)]` turns a normal struct into a typed argv parser backed by
one clap `Command`. It has no Serde dependency.

The derive-generated implementation starts from `Self::default()` and applies
argv assignments in order, so the last assignment to a field wins.

### `Config`

`#[derive(kwconf::Config)]` adds config files and declared environment bindings.
Its source order is:

```text
defaults < config file < env < argv
```

Resolution also starts from `Self::default()` and applies typed fields directly.
The config struct itself is not serialized into an intermediate JSON object and
does not need to implement `Serialize` or `Deserialize`.

Serde remains an internal leaf-value decoding mechanism for the full `Config`
layer. A custom leaf type used with `Config` must implement `Deserialize`; the
outer config and nested `#[kwconf(subconfig)]` structs do not.

## Defaults

`#[kwconf(default = EXPR)]` sets a field default. Otherwise kwconf uses
`Default::default()` for that field.

The derive implements `Default` for the config/CLI struct.

## CLI grammar ownership

One clap command model (implemented with `clap_builder`) owns:

- argv recognition;
- long-option aliases;
- dash/underscore normalization;
- bool negation flags;
- modal subcommands;
- help;
- color policy; and
- completion generation.

Kwconf does not maintain a second handwritten argv parser.

## Names

Field `some_value` is exposed canonically as `--some-value` and accepts the
underscore spelling as an alias. Dotted subconfig paths apply the same rule to
each component.

Schema collisions are rejected before parsing. This includes canonical/alias
collisions, dash/underscore-equivalent names, bool negation collisions, modal
variant alias collisions, and enabled special-option names.

## Boolean values

Boolean text accepts exactly:

```text
true false 1 0
```

`true`/`false` matching is ASCII-case-insensitive. `yes/no` and `on/off` are not
part of the contract.

A direct `bool` field also receives a valueless positive flag and a generated
negation flag:

```text
--cache
--no-cache
```

The last assignment in argv wins.

## Lightweight CLI scalar parsing

`Cli` uses ordinary Rust `FromStr` for scalar fields. In addition:

- `bool` uses the strict spellings above;
- `Option<T>` treats `none` and `null` as `None`;
- `Vec<T>` uses `parser = "csv"` and parses every component through `T::from_str`;
- `parser = "yaml"` is rejected because it belongs to the Serde-backed config layer;
- env bindings and `special_options(config)` are rejected because `Cli` is argv-only.

This keeps the derive-only dependency path free of Serde.

## Full config raw parsing

For argv/env values in `Config`, parsing is destination-type-directed through
Serde's deserializer interface.

### `auto`

- `String`: original text.
- `bool`: strict kwconf boolean spellings.
- integer/float types: trimmed numeric parsing.
- `Option<T>`: `null`/`none` => `None`.
- unit enum: bare variant spelling.
- sequence/map/struct: structured JSON where the destination asks for it.
- untyped `serde_json::Value`: infer bool, number, null, JSON arrays/objects;
  otherwise keep a string.

### `csv`

Comma-separated fields are trimmed and decoded by element type. Empty fields
are preserved rather than filtered:

```text
a,,b, -> ["a", "", "b", ""]
```

Therefore an empty input is one empty `String` element, while an empty numeric
component fails numeric parsing. This is an intentional divergence from Python
kwconf's current empty-component filtering.

### `yaml`

With the `yaml` Cargo feature, YAML is parsed before destination conversion.
Malformed YAML is always an error; a `String` field does not turn malformed
YAML back into the raw token. Without that feature, requesting `parser =
"yaml"` returns a feature-required error.

## Config files

The `config` feature always supports `.json`. Format features add:

- `toml` (enabled by default): `.toml`;
- `yaml` (opt-in): `.yaml` and `.yml`.

Requesting a disabled format reports the Cargo feature needed to enable it.
Unknown extensions are tried against every format enabled in that build and
report those parser failures when none succeeds.

Config keys use kwconf field names and aliases, with dash/underscore
normalization. They do not depend on Serde rename attributes on the outer config
struct because kwconf no longer deserializes that struct wholesale.

Nested subconfigs use nested objects/tables or dotted paths.

## Environment

Environment bindings are opt-in per field:

```rust
#[kwconf(env = "TRAIN_LR")]
lr: f64,
```

Only declared names are queried from the process. Explicit values supplied via
`Sources` override the process environment.

`Config::from_iter` is argv-only. `Config::try_cli` reads current argv plus
declared process-environment bindings. This makes explicit test/programmatic
argv deterministic.

## Modal commands

`ModalCli` and `ModalConfig` are enums whose variants wrap one payload struct.
Both use clap subcommands. `ModalConfig` additionally allows config files to
select the variant through `command` or `mode` and to store payloads under
variant tables.

## Features

```toml
# No proc macros, no Serde
kwconf = { version = "0.1", default-features = false }

# Python-like CLI struct API, one kwconf proc-macro layer, no Serde
kwconf = { version = "0.1", default-features = false, features = ["derive"] }

# Normal layered config API: derive + config + JSON + TOML
kwconf = "0.1"

# Add YAML/YML
kwconf = { version = "0.1", features = ["yaml"] }

# Add shell-completion generation
kwconf = { version = "0.1", features = ["completion"] }

# All supported capabilities
kwconf = { version = "0.1", features = ["full"] }
```

The default features are `derive`, `config`, and `toml`. `config` uses
`serde_core`/`serde_json`; YAML and `clap_complete` are not default costs.

## Help and completion

`--help` is built in. `special_options(color)` enables
`--color auto|always|never`. `special_options(generate_completion)` enables
`--generate-completion SHELL`.

Supported completion shells are bash, elvish, fish, PowerShell, and zsh.
Actual completion generation requires the `completion` Cargo feature. Without
it, an enabled `--generate-completion` option and the direct
`try_completion_script(...)` API report that the feature is required. The
`completion_script(...)` convenience wrapper panics on that error. Keeping the
methods present in every feature tier ensures proc-macro expansions do not
depend on Cargo features of the downstream crate.
