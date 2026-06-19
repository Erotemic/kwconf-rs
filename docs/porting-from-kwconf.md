# Porting from Python kwconf

This crate favors a direct mechanical port.

## Imports

Python:

```python
import kwconf
```

Rust:

```rust
use serde::{Deserialize, Serialize};
```

The Rust derive path stays explicit:

```rust
#[derive(Serialize, Deserialize, kwconf::Config)]
```

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
#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
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
| env binding | `#[kwconf(env = "NAME")]` |

## Source precedence

Keep the same mental model:

```text
defaults < config file < env < argv
```

Config files are structured. Env and argv are strings, so parser metadata matters
there.

## Parser mapping

| kwconf parser | kwconf-rs parser | Typical Rust field |
| --- | --- | --- |
| `auto` | `auto` | `String`, `bool`, numbers |
| `csv` | `csv` | `Vec<String>` |
| `yaml` | `yaml` | `Vec<T>`, maps, nested structs |

## CLI entrypoint

Python:

```python
cfg = TrainConfig.cli()
```

Rust:

```rust
let cfg = TrainConfig::cli();
```

Use `try_cli()` or `from_iter(...)` in tests.

## Help colors

Python kwconf can use `rich_argparse` when it is installed.

Rust kwconf-rs uses `clap` help rendering with an explicit style palette:

```rust
let help = TrainConfig::help_with_color(kwconf::ColorChoice::Auto);
```

The CLI also accepts:

```bash
train --color always --help
train --color never --help
```

`TrainConfig::cli()` uses `auto` by default. Tests can use `Never` for stable
snapshots or `Always` to assert ANSI output.

## Completion

Python kwconf can use `argcomplete` when it is installed.

Rust kwconf-rs emits completion scripts:

```bash
train --generate-completion bash > train.bash
train --generate-completion zsh > _train
```

The same output is available from Rust:

```rust
let script = TrainConfig::completion_script(kwconf::CompletionShell::Bash, "train");
```

## Parity demo

The repo includes both versions of the same CLI:

- `examples/parity/kwconf_train.py`
- `crates/kwconf/examples/kwconf_rs_train.rs`
- `crates/kwconf/tests/parity.rs`

The tests cover source precedence, parser behavior, generated help, and generated
completion content.

## Current gaps

The first Rust crate does not attempt full Python parity.

Deferred areas:

- nested config ergonomics;
- modal / subcommand config;
- richer provenance reporting;
- deeper clap interop.

Add features when real ports need them.
