# kwconf-rs contract

This repo starts a Rust implementation of the kwconf CLI/config contract.

## Source order

Values resolve in this order:

```text
defaults < config file < env < argv
```

A later source wins for the same field.

## Defaults

`#[kwconf(default = EXPR)]` sets the default value for a field.

Fields without an explicit default use `Default::default()`.

## Config files

`--config PATH` loads a structured file before env and argv.

Supported file extensions:

- `.toml`
- `.json`
- `.yaml`
- `.yml`

Config files use field names as keys. Dashes and underscores are treated the
same at source boundaries.

## Env

Env is opt-in per field:

```rust
#[kwconf(env = "TRAIN_TAGS")]
tags: Vec<String>,
```

Env values are strings, so the field parser is used.

## Argv

Argv accepts long options:

```text
--name value
--name=value
--bool-flag
```

Boolean flags without an explicit value receive `true`.

`--config`, `--help`, `--color`, and `--generate-completion` are reserved by the runtime.

## Parsers

Parsers only apply to string-only sources: env and argv.

### auto

`auto` parses common scalar values:

- `true` and `false`
- integers and floats
- `null` and `none`
- JSON arrays and objects

Other values stay strings.

### csv

`csv` splits a comma-separated string into an array of strings.

```text
--tags=red,blue
```

becomes:

```text
["red", "blue"]
```

### yaml

`yaml` parses a YAML scalar, sequence, or mapping from a string.

Use it when a field needs structured data from env or argv.

## Choices

`choices` validates string values before deserialization.

```rust
#[kwconf(default = "fast", choices = ["fast", "safe"])]
mode: String,
```

## Help and completion

`kwconf-rs` builds help and completion metadata from the same field spec.

- `Config::help()` renders normal help.
- `--color auto|always|never` controls color for CLI help.
- `Config::help_with_color(...)` renders help with an explicit color policy.
- `--generate-completion SHELL` prints a completion script.
- `Config::completion_script(...)` returns the script as a string.

Supported completion shells are `bash`, `elvish`, `fish`, `powershell`, and
`zsh`.
