# Porting from Python kwconf

The Rust port keeps the configuration-object model but separates lightweight
CLI use from layered configuration.

## Pick the layer first

If the Python class is only being used to define argv, use `Cli`:

```rust
#[derive(Debug, kwconf::Cli)]
struct TrainArgs {
    #[kwconf(default = 0.001)]
    lr: f64,

    #[kwconf(parser = "csv")]
    tags: Vec<String>,
}
```

This path has no Serde dependency.

If the Python class uses config files or environment bindings, use `Config`:

```rust
#[derive(Debug, kwconf::Config)]
struct TrainConfig {
    #[kwconf(default = 0.001, env = "TRAIN_LR")]
    lr: f64,

    #[kwconf(parser = "csv", env = "TRAIN_TAGS")]
    tags: Vec<String>,
}
```

The outer Rust config struct does not need Serde derives.

## Config classes become structs

Python:

```python
class TrainConfig(kwconf.Config):
    lr = 0.001
    mode = kwconf.Value('fast', choices=['fast', 'safe'])
    tags = kwconf.Value(default_factory=list, parser='csv')
```

Rust:

```rust
#[derive(Debug, kwconf::Config)]
struct TrainConfig {
    #[kwconf(default = 0.001)]
    lr: f64,

    #[kwconf(default = "fast", choices = ["fast", "safe"])]
    mode: String,

    #[kwconf(parser = "csv")]
    tags: Vec<String>,
}
```

## `Value(...)` metadata becomes field attributes

| Python kwconf | kwconf-rs |
| --- | --- |
| `default=...` | `#[kwconf(default = ...)]` |
| `choices=[...]` | `#[kwconf(choices = [...])]` |
| `parser='csv'` | `#[kwconf(parser = "csv")]` |
| `alias='foo'` | `#[kwconf(alias = "foo")]` |
| env binding | `#[kwconf(env = "NAME")]` on `Config` |

Rust doc comments are used for help text when `help = ...` is not supplied, so
ordinary Rust documentation can usually replace repeated help strings.

## Source precedence

Full `Config` keeps:

```text
defaults < config file < env < argv
```

`Cli` is intentionally argv-only.

`Config::from_iter(...)` is also argv-only for deterministic programmatic use.
`Config::try_cli()` uses current argv plus declared process-environment
bindings. Use `Sources` when tests or callers need an explicit config/env mix.

## Parser mapping

| kwconf parser | kwconf-rs | Notes |
| --- | --- | --- |
| `auto` | `auto` | type-directed in `Config`, `FromStr` scalars in `Cli` |
| `csv` | `csv` | typed `Vec<T>`; empty components are preserved in Rust |
| `yaml` | `yaml` | full `Config` only; requires Cargo feature `yaml` |

Python kwconf currently filters empty CSV components after trimming. Rust does
not copy that behavior. For example:

```text
a,,b, -> ["a", "", "b", ""]
```

This keeps the information expressed by the delimiters.

Boolean text in Rust is deliberately limited to `true/false` and `1/0`.

## CLI entrypoint

Python:

```python
cfg = TrainConfig.cli()
```

Rust:

```rust
let cfg = TrainConfig::cli();
```

The same call shape works for `Cli` and `Config` derives.

## Nested configs

Python nested config objects map to nested Rust structs.

```rust
#[derive(Debug, kwconf::Config)]
struct OptimizerConfig {
    #[kwconf(default = 0.001)]
    lr: f64,
}

#[derive(Debug, kwconf::Config)]
struct JobConfig {
    #[kwconf(subconfig)]
    optimizer: OptimizerConfig,
}
```

Config files use nested tables. CLI flags use dotted paths:

```bash
job --optimizer.lr=0.02
```

Use `kwconf::Cli` on both structs for the argv-only version.

## Modal configs

Python modal CLIs map to Rust enums.

Argv-only:

```rust
#[derive(Debug, kwconf::ModalCli)]
enum KwTool {
    Train(TrainArgs),
    Eval(EvalArgs),
}
```

Layered:

```rust
#[derive(Debug, kwconf::ModalConfig)]
enum KwTool {
    #[kwconf(default)]
    Train(TrainConfig),

    #[kwconf(alias = "test")]
    Eval(EvalConfig),
}
```

Both are clap subcommands. `ModalConfig` additionally permits config files to
select a mode with `command` or `mode`.

## Custom Rust field types

For lightweight `Cli`, scalar custom types should implement `FromStr`.

For full `Config`, custom leaf types should implement/derive Serde
`Deserialize`. The outer kwconf config struct and nested subconfig structs do
not need to do so merely to participate in kwconf.

## Help and completion

`#[kwconf(special_options(color))]` enables:

```bash
train --color always --help
```

With Cargo feature `completion`, `#[kwconf(special_options(generate_completion))]`
enables:

```bash
train --generate-completion bash > train.bash
```

Only full `Config` / `ModalConfig` may enable `special_options(config)`.

## Dependency choices

```toml
# Normal layered configuration: JSON + TOML
kwconf = "0.1"

# Add YAML and/or shell completion generation as needed
kwconf = { version = "0.1", features = ["yaml", "completion"] }

# Python-like argv-only API with no Serde
kwconf = { version = "0.1", default-features = false, features = ["derive"] }

# No kwconf proc macro and no Serde
kwconf = { version = "0.1", default-features = false }
```

For a hand-built CLI with no interest in kwconf's config-object semantics, use
clap's builder API directly rather than treating kwconf as a competing CLI
builder.
