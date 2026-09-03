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

When enabled with `#[kwconf(special_options(config))]`, `--config PATH` loads a structured file before env and argv. Programmatic callers can still pass a config path through `Sources` without enabling the CLI flag.

Supported file extensions:

- `.toml`
- `.json`
- `.yaml`
- `.yml`

Config files use field names as keys. Dashes and underscores are treated the
same at source boundaries.

Nested subconfigs use nested tables:

```toml
width = 128

[optimizer]
lr = 0.01
kind = "sgd"
```

Modal config files select a subcommand with `command` or `mode` and keep each
variant under its own table:

```toml
command = "train"

[train]
lr = 0.01
```

## Env

Env is opt-in per field:

```rust
#[kwconf(env = "TRAIN_TAGS")]
tags: Vec<String>,
```

Env values are strings, so the field parser is used. Nested env bindings live on
the nested field.

## Argv

Argv accepts long options:

```text
--name value
--name=value
--bool-flag
```

Boolean flags without an explicit value receive `true`.

Nested subconfigs use dotted paths:

```text
--optimizer.lr=0.02
```

Modal subcommands use the normal command shape:

```text
kwtool train --lr=0.02
```

`--help` is always handled by the runtime. Other runtime flags are opt-in so applications can still use ordinary fields named `config`, `color`, or `generate_completion` when they want to.

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, kwconf::Config)]
#[kwconf(name = "train", special_options(config, color, generate_completion))]
struct TrainConfig {
    // ...
}
```

For modal CLIs, root runtime flags go before the subcommand. If the selected subcommand config also enables a runtime flag, that flag can appear after the subcommand with the rest of the subcommand fields.

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

## Nested subconfigs

Mark nested structs with `#[kwconf(subconfig)]`.

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, kwconf::Config)]
struct JobConfig {
    #[kwconf(default = 64)]
    width: usize,

    #[kwconf(subconfig)]
    optimizer: OptimizerConfig,
}
```

Nested config fields appear in help and completions as dotted flags.

## Modal subcommands

Mark an enum with `#[derive(kwconf::ModalConfig)]`. Each variant wraps one
`kwconf::Config` payload.

```rust
#[derive(Debug, Clone, kwconf::ModalConfig)]
enum KwTool {
    #[kwconf(default, help = "Run training.")]
    Train(TrainConfig),

    #[kwconf(alias = "test", help = "Run evaluation.")]
    Eval(EvalConfig),
}
```

The default variant is used when argv and config do not select one.

## Help and completion

`kwconf-rs` builds help and completion metadata from the same field spec.

- `Config::help()` renders normal help.
- `ModalConfig::help()` renders modal help.
- `--color auto|always|never` controls color for CLI help when `special_options(color)` is enabled.
- `help_with_color(...)` renders help with an explicit color policy without requiring the CLI flag.
- `--generate-completion SHELL` prints a completion script when `special_options(generate_completion)` is enabled.
- `completion_script(...)` returns the script as a string without requiring the CLI flag.

Supported completion shells are `bash`, `elvish`, `fish`, `powershell`, and
`zsh`.
