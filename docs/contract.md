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

Only declared variables are read, one at a time, so an unrelated variable with
non-Unicode content never affects a run. A declared variable that is not valid
UTF-8 is an error. `Sources::new()`, `Sources::from_iter(...)`, `cli()`,
`try_cli()`, and `from_iter(...)` read the process environment;
`Sources::empty()` does not unless `with_process_env(true)` is set. Explicit
`with_env(...)` bindings win over the process environment.

## Argv

Argv is recognized by one `clap` command model that is also used for help and
completion scripts, so the three never disagree.

Argv accepts long options:

```text
--name value
--name=value
--bool-flag
--no-bool-flag
```

Boolean flags without an explicit value receive `true`; `--no-flag` sets the
field to `false`. When the same field is assigned more than once, the last
assignment in argv wins, including across `--flag` and `--no-flag`.

Dashes and underscores are interchangeable in option names, in any mix.
Aliases apply to every component of a dotted path, so with a subconfig field
`optimizer` aliased to `opt` and a leaf `learning_rate` aliased to `lr`,
`--opt.lr=1` is accepted. Help lists the canonical spelling, its underscore
form, and leaf aliases.

Unknown options, stray positionals, and anything after `--` are errors.
`-h` and `--help` are always available.

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

For modal CLIs, root runtime flags go before the subcommand. If the selected subcommand config also enables a runtime flag, that flag can appear after the subcommand with the rest of the subcommand fields. A root `--config` file contributes the selected variant's table; a subcommand `--config` file is layered on top of it.

The schema is validated before any parsing. Two fields, aliases, negations, or
special options that would claim the same option (compared
dash/underscore-insensitively) are a compile error inside one struct and an
`Error::Schema` when the collision spans nested subconfigs. `help` is always
reserved.

## Parsers

Parsers only apply to string-only sources: env and argv.

argv and env text is kept verbatim until the final deserialization, and the
destination field type decides how it is read. `--label=123` stays `"123"` for
a `String` field and becomes `123` for a `u32` field. This mirrors Python
kwconf, where `auto('123', str)` stays a string.

### auto

`auto` coerces text by the destination type:

- `String`: the text as written.
- `bool`: `true`/`false`, `yes`/`no`, `on`/`off`, `1`/`0` (case-insensitive).
- integers and floats: parsed from the trimmed text.
- `Option<T>`: `null` and `none` are `None`; anything else is `Some`.
- unit enums: the text is the variant name (after serde renames).
- `Vec<T>`, maps, and structs: a JSON array or object.
- `serde_json::Value` and other untyped destinations: `true`/`false`, integers,
  floats, `null`/`none`, JSON arrays and objects are inferred; other values
  stay strings.

### csv

`csv` splits a comma-separated string and coerces each element by the element
type, so `Vec<i64>` receives integers and `Vec<String>` receives strings.

```text
--tags=red,blue
```

becomes:

```text
["red", "blue"]
```

An empty string is an empty list.

### yaml

`yaml` parses a YAML scalar, sequence, or mapping from a string and then
deserializes the result into the field. A `String` field still receives the
text of a scalar.

Use it when a field needs structured data from env or argv.

Deserialization errors name the dotted field path, the offending text, and its
source (`argv` or `env`).

## Choices

`choices` validates values before deserialization. argv and env text is
compared as written; config values are compared as strings, numbers, or
booleans.

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

`kwconf-rs` builds help, completion scripts, and argv parsing from one `clap`
command model.

- `Config::help()` renders normal help.
- `ModalConfig::help()` renders modal help.
- `--help` inside `cli()` prints help and exits `0`; `try_cli()` returns
  `Error::HelpRequested(Help)` with plain and ANSI text plus the requested
  color policy.
- `--color auto|always|never` controls color for CLI help when `special_options(color)` is enabled.
- `help_with_color(...)` renders help with an explicit color policy without requiring the CLI flag.
- `--generate-completion SHELL` prints a completion script when `special_options(generate_completion)` is enabled.
- `completion_script(...)` returns the script as a string without requiring the CLI flag.

Supported completion shells are `bash`, `elvish`, `fish`, `powershell`, and
`zsh`.
