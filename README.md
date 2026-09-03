# kwconf-rs

`kwconf-rs` is a small Rust crate for porting kwconf-style Python CLIs to Rust.

It keeps the kwconf contract:

1. defaults are the base layer;
2. config files override defaults;
3. environment variables override config files;
4. argv overrides everything;
5. parsers only apply to string sources such as argv and env.

The first implementation is intentionally narrow. It supports derived structs,
TOML / JSON / YAML config files, explicit env bindings, nested subconfigs, modal subcommands, generated help, colored
help via `clap`, completion scripts via `clap_complete`, and the parser names
`auto`, `csv`, and `yaml`.

## Example

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "train", about = "Train a model.")]
struct TrainConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(default = "fast", choices = ["fast", "safe"], help = "Run mode.")]
    mode: String,

    #[kwconf(parser = "csv", env = "TRAIN_TAGS", help = "Comma-separated tags.")]
    tags: Vec<String>,
}

fn main() {
    let cfg = TrainConfig::cli();
    println!("{cfg:#?}");
}
```

Run the bundled examples:

```bash
cargo run -p kwconf --example basic -- --help
cargo run -p kwconf --example basic -- --color always --help
cargo run -p kwconf --example basic -- --config examples/basic.toml --lr=0.01 --tags=red,blue
TRAIN_TAGS=nightly,smoke cargo run -p kwconf --example basic -- --mode=safe
cargo run -p kwconf --example kwconf_rs_train -- --config examples/parity/train.toml --lr=0.01 --tags=argv,override
cargo run -p kwconf --example nested -- --config examples/nested.toml --optimizer.lr=0.02
cargo run -p kwconf --example modal -- --config examples/modal.toml train --lr=0.02
```

Generate shell completions:

```bash
cargo run -p kwconf --example kwconf_rs_train -- --generate-completion bash > train.bash
cargo run -p kwconf --example kwconf_rs_train -- --generate-completion zsh > _train
cargo run -p kwconf --example kwconf_rs_train -- --generate-completion fish > train.fish
```

## Python kwconf parity demo

The repo includes both sides of the same demo:

- `examples/parity/kwconf_train.py` uses Python `kwconf`.
- `crates/kwconf/examples/kwconf_rs_train.rs` uses `kwconf-rs`.
- `crates/kwconf/tests/parity.rs` tests the Rust side against the shared contract.

The shared behavior is:

```text
defaults < config file < env < argv
```

The Python shape:

```python
import kwconf


class TrainConfig(kwconf.Config):
    lr = kwconf.Value(0.001, help='Learning rate.')
    mode = kwconf.Value('fast', choices=['fast', 'safe'], help='Run mode.')
    tags = kwconf.Value(default_factory=list, parser='csv', help='Comma-separated tags.')
```

The Rust shape:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, kwconf::Config)]
struct TrainConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(default = "fast", choices = ["fast", "safe"], help = "Run mode.")]
    mode: String,

    #[kwconf(parser = "csv", env = "TRAIN_TAGS", help = "Comma-separated tags.")]
    tags: Vec<String>,
}
```


## Nested subconfigs

Use `#[kwconf(subconfig)]` for a field that is another `kwconf::Config`.
Config files use nested tables. CLI flags use dotted paths.

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, kwconf::Config)]
struct OptimizerConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, kwconf::Config)]
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
cargo run -p kwconf --example nested -- --config examples/nested.toml --optimizer.lr=0.02
```

Nested env bindings live on the nested fields. The same precedence applies:

```text
defaults < config file < env < argv
```

## Modal subcommands

Use `#[derive(kwconf::ModalConfig)]` on an enum. Each variant wraps one
`kwconf::Config` payload.

```rust
#[derive(Debug, Clone, kwconf::ModalConfig)]
#[kwconf(name = "kwtool", about = "Modal CLI demo.")]
enum KwTool {
    #[kwconf(default, help = "Run training.")]
    Train(TrainConfig),

    #[kwconf(alias = "test", help = "Run evaluation.")]
    Eval(EvalConfig),
}

let command = KwTool::cli();
```

Run a subcommand directly:

```bash
cargo run -p kwconf --example modal -- train --lr=0.02 --tags=cli,tag
```

Or select it from a modal config file:

```toml
command = "train"

[train]
lr = 0.01
tags = ["file", "demo"]
```

```bash
cargo run -p kwconf --example modal -- --config examples/modal.toml train --lr=0.02
```

Modal root runtime flags such as `--config`, `--color`, and `--generate-completion`
go before the subcommand when they are enabled on the modal enum. Subcommand
runtime flags can also be enabled on the payload config and then placed after the
subcommand with the other subcommand fields.

## Parsers

Parsers convert string-only sources into typed config values.

- `auto` parses booleans, numbers, `null`, JSON arrays, and JSON objects. Other
  values stay strings.
- `csv` splits a comma-separated string into an array of strings.
- `yaml` parses a YAML scalar, sequence, or mapping.

Config files are already structured sources. Their values deserialize directly
into the target struct.

## Source precedence

`kwconf-rs` resolves values in this order:

```text
defaults < config file < env < argv
```

`--help` is always handled by the runtime. The `--config PATH`, `--color WHEN`,
and `--generate-completion SHELL` flags are opt-in special options so projects
can still use ordinary fields named `config`, `color`, or `generate_completion`.
Config files may be TOML, JSON, YAML, or YML.

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, kwconf::Config)]
#[kwconf(name = "train", special_options(config, color, generate_completion))]
struct TrainConfig {
    // fields...
}
```


## Colored help and completions

`kwconf-rs` uses Rust-native CLI tooling for optional polish.

- Help rendering is backed by `clap` with a Cargo-like style palette.
- `#[kwconf(special_options(color))]` enables `--color auto|always|never`.
- `Config::help_with_color(...)` lets callers choose `Auto`, `Always`, or `Never`.
- `#[kwconf(special_options(generate_completion))]` enables `--generate-completion SHELL`.
- `Config::completion_script(...)` exposes the same behavior to build scripts or tests.

Use `--color always --help` to force ANSI color through terminals, logs, or test
harnesses that do not look like a TTY.

Supported completion shells are `bash`, `elvish`, `fish`, `powershell`, and `zsh`.

## Porting from Python kwconf

A common Python config:

```python
import kwconf


class TrainConfig(kwconf.Config):
    lr = 0.001
    mode = kwconf.Value('fast', choices=['fast', 'safe'])
    tags = kwconf.Value(default_factory=list, parser='csv')


cfg = TrainConfig.cli()
```

Maps to:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, kwconf::Config)]
struct TrainConfig {
    #[kwconf(default = 0.001)]
    lr: f64,

    #[kwconf(default = "fast", choices = ["fast", "safe"])]
    mode: String,

    #[kwconf(parser = "csv")]
    tags: Vec<String>,
}

let cfg = TrainConfig::cli();
```

See `docs/porting-from-kwconf.md` for more cases.

## Status

This is a repo starter, not a crates.io release. The API should stay small until
real ports expose the repeated patterns.
