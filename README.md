# kwconf-rs

`kwconf-rs` is a small Rust crate for porting kwconf-style Python CLIs to Rust.

It keeps the kwconf contract:

1. defaults are the base layer;
2. config files override defaults;
3. environment variables override config files;
4. argv overrides everything;
5. parsers only apply to string sources such as argv and env.

The first implementation is intentionally narrow. It supports derived structs,
TOML / JSON / YAML config files, explicit env bindings, generated help, colored
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

The `--config PATH`, `--color WHEN`, and `--generate-completion SHELL` flags are
reserved by the runtime. Config files may be TOML, JSON, YAML, or YML.

## Colored help and completions

`kwconf-rs` uses Rust-native CLI tooling for optional polish.

- Help rendering is backed by `clap` with a Cargo-like style palette.
- `--color auto|always|never` controls color for help output.
- `Config::help_with_color(...)` lets callers choose `Auto`, `Always`, or `Never`.
- `--generate-completion SHELL` emits a completion script.
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
