# kwconf-rs

`kwconf-rs` brings the Python `kwconf` style to Rust: define a normal typed
struct, annotate the configuration semantics once, and use the same shape from
a CLI or from layered config sources.

The crate deliberately separates two use cases:

- `#[derive(kwconf::Cli)]` is the lightweight argv-only API. It uses `clap`
  underneath, needs no Serde dependency, and gives you a typed struct directly.
- `#[derive(kwconf::Config)]` adds config files and environment variables with
  the precedence contract `defaults < config file < env < argv`.

The derive macros generate metadata and typed field setters. `clap` remains the
single implementation of command-line grammar, help, aliases, subcommands, and
completions.

## Lightweight CLI

```rust
#[derive(Debug, kwconf::Cli)]
/// Train a model.
struct TrainArgs {
    /// Learning rate.
    #[kwconf(default = 0.001)]
    lr: f64,

    /// Enable verbose logging.
    verbose: bool,

    /// Comma-separated tags.
    #[kwconf(parser = "csv")]
    tags: Vec<String>,
}

fn main() {
    let args = TrainArgs::cli();
    println!("{args:#?}");
}
```

```bash
train --lr=0.01 --verbose --tags=nightly,smoke
```

For this mode, use:

```toml
kwconf = { version = "0.1", default-features = false, features = ["derive"] }
```

That dependency path has no Serde dependency. It also does not enable clap's
derive macros; kwconf has one proc-macro layer of its own for the struct API.

If you disable `derive` as well, the runtime contains no proc-macro dependency:

```toml
kwconf = { version = "0.1", default-features = false }
```

The public `Cli` trait remains implementable manually, but kwconf does not try
to replace clap's mature builder API. If all you want is a hand-built CLI,
using clap directly is usually the better choice.

## Layered config

Full `Config` adds environment and config-file sources:

```rust
#[derive(Debug, kwconf::Config)]
#[kwconf(name = "train", special_options(config))]
struct TrainConfig {
    /// Learning rate.
    #[kwconf(default = 0.001, env = "TRAIN_LR")]
    lr: f64,

    #[kwconf(default = "fast", choices = ["fast", "safe"])]
    mode: String,

    #[kwconf(parser = "csv", env = "TRAIN_TAGS")]
    tags: Vec<String>,
}

fn main() {
    let cfg = TrainConfig::cli();
    println!("{cfg:#?}");
}
```

```text
defaults
    < config file
    < environment
    < argv
```

```bash
TRAIN_LR=0.005 train --config=train.toml --tags=nightly,smoke
```

The config struct itself does **not** need `Serialize` or `Deserialize`.
Resolution starts from `TrainConfig::default()` and applies each winning field
value directly. Serde is used internally to decode full-`Config` leaf values.
Primitive and standard-library field types already have the needed support; a
custom leaf type used with `Config` must implement `Deserialize`. That
requirement stays local to the leaf type instead of forcing Serde derives onto
the outer config struct or its subconfig structs.

The default feature set is the convenient full API:

```toml
kwconf = "0.1"
```

Equivalent explicit features are:

```toml
kwconf = { version = "0.1", features = ["derive", "config"] }
```

## Nested configs

Use `#[kwconf(subconfig)]` for another kwconf struct. CLI flags use dotted
paths; config files use nested objects/tables.

```rust
#[derive(Debug, kwconf::Config)]
struct OptimizerConfig {
    #[kwconf(default = 0.001)]
    lr: f64,
}

#[derive(Debug, kwconf::Config)]
struct JobConfig {
    #[kwconf(default = 64)]
    width: usize,

    #[kwconf(subconfig)]
    optimizer: OptimizerConfig,
}
```

```toml
width = 128

[optimizer]
lr = 0.01
```

```bash
job --optimizer.lr=0.02
```

The lightweight API supports the same shape with `kwconf::Cli` on both
structs.

## Modal/subcommand CLIs

The modal forms are enums whose variants wrap one payload struct.

Lightweight:

```rust
#[derive(Debug, kwconf::ModalCli)]
enum Command {
    Train(TrainArgs),
    Eval(EvalArgs),
}
```

Layered config:

```rust
#[derive(Debug, kwconf::ModalConfig)]
enum Command {
    Train(TrainConfig),
    Eval(EvalConfig),
}
```

Both are backed by clap subcommands, including aliases, nested help, and
completion generation.

## Parsers

String-only sources are decoded according to the destination type.

### `auto`

For full `Config`, `auto` is type-directed:

- `String` keeps the input spelling, so `--label=123` remains `"123"`.
- booleans accept `true`, `false`, `1`, and `0` case-insensitively for words.
- integers and floats parse as their destination types.
- `Option<T>` treats `null` and `none` as `None`.
- unit enums take their variant spelling.
- structured collection/object destinations can take JSON.
- an untyped `serde_json::Value` infers booleans, numbers, null, arrays, and
  objects.

Lightweight `Cli` uses ordinary Rust `FromStr` for scalar values, with the same
strict bool and option-null behavior provided by kwconf.

### `csv`

`csv` splits on commas, trims surrounding whitespace, and parses each element
according to the element type.

Explicit empty fields are preserved:

```text
a,,b,  ->  ["a", "", "b", ""]
```

and an empty input is one explicit empty field for `Vec<String>`:

```text
--tags=  ->  [""]
```

This intentionally differs from Python kwconf's current behavior, which drops
empty CSV components after trimming. Rust treats delimiters as data boundaries
rather than erasing explicitly present fields.

### `yaml`

`yaml` is available on full `Config`, where Serde-backed structured decoding is
already enabled. Malformed YAML is an error even for a `String` destination; it
does not fall back to the original token.

## Boolean flags

A `bool` field accepts:

```text
--cache
--cache=true
--cache=false
--cache=1
--cache=0
--no-cache
```

The last argv assignment wins. Spellings such as `yes/no` and `on/off` are not
accepted.

## Special options

`--help` is always present. Other runtime options are opt-in so ordinary fields
may still use those names:

```rust
#[kwconf(special_options(color, generate_completion))]
```

Full `Config` can additionally opt into:

```rust
#[kwconf(special_options(config))]
```

or combine them:

```rust
#[kwconf(special_options(config, color, generate_completion))]
```

`Cli` / `ModalCli` reject `special_options(config)` at derive time.

## Config files

With the `config` feature, TOML, JSON, YAML, and YML files are supported.
Unknown extensions are tried as TOML, JSON, and YAML, with all parser failures
reported if none succeeds.

Only declared environment bindings are queried from the process, so unrelated
non-Unicode environment entries cannot crash argument parsing. `--config`
paths remain `PathBuf`s and may be non-UTF-8 on platforms that permit it.

## Public API and dependency tiers

The intended public surface is small:

- `Cli`, `ModalCli`
- `Config`, `ModalConfig`, `Sources` when `config` is enabled
- `Error`, `Help`, `Result`
- `ColorChoice`, `CompletionShell`
- the matching derive macros when `derive` is enabled

Derive plumbing lives under `kwconf::__private` and is not stable API.

The dependency tiers are deliberate:

```text
kwconf --no-default-features
    clap runtime only
    no Serde
    no proc-macro dependency

kwconf --no-default-features --features derive
    + kwconf_derive
    + syn / quote / proc-macro2
    no Serde

kwconf (default features)
    + layered config support
    + Serde runtime / JSON / TOML / YAML
```

Clap's derive feature is not enabled underneath kwconf, so the convenient API
does not stack a second clap proc-macro layer.

## Status

`0.1.0` is the first release. The minimum supported Rust version is 1.85.
See `docs/contract.md`, `docs/porting-from-kwconf.md`, and `CHANGELOG.md` for the
behavioral contract and migration details.
