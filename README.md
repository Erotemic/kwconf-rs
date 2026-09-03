# kwconf-rs

`kwconf-rs` is a small Rust implementation of the `kwconf` configuration style:
define a typed configuration object once, then construct it from defaults,
config files, environment variables, and command-line arguments.

The core precedence contract is:

```text
defaults < config file < env < argv
```

The same definition can also be used as a lightweight CLI without config-file
or environment support.

`kwconf-rs` supports derived structs, TOML / JSON config files by default,
optional YAML config files, explicit environment bindings, nested configs,
modal subcommands, generated help, colored help via clap, optional shell
completions, and the parser modes `auto`, `csv`, and optional `yaml`.

## Python kwconf to Rust

The main goal is to preserve the shape and ergonomics of Python `kwconf` while
using ordinary typed Rust structs.

Python:

```python
import kwconf


class TrainConfig(kwconf.Config):
    lr = kwconf.Value(0.001, help='Learning rate.')
    mode = kwconf.Value('fast', choices=['fast', 'safe'], help='Run mode.')
    tags = kwconf.Value(
        default_factory=list,
        parser='csv',
        help='Comma-separated tags.',
    )
```

Rust:

```rust
#[derive(Debug, kwconf::Config)]
#[kwconf(name = "train", about = "Train a model.", special_options(config))]
struct TrainConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(
        default = "fast",
        choices = ["fast", "safe"],
        help = "Run mode."
    )]
    mode: String,

    #[kwconf(
        parser = "csv",
        env = "TRAIN_TAGS",
        help = "Comma-separated tags."
    )]
    tags: Vec<String>,
}

fn main() {
    let cfg = TrainConfig::cli();
    println!("{cfg:#?}");
}
```

The repo includes both sides of this demo:

- `examples/parity/kwconf_train.py` uses Python `kwconf`.
- `crates/kwconf/examples/kwconf_rs_train.rs` uses `kwconf-rs`.
- `crates/kwconf/tests/parity.rs` tests the Rust behavior against the shared
  contract.
- `docs/side-by-side-parity-demo.md` contains the extended side-by-side demo.

Run the Rust side with layered sources:

```bash
cargo run -p kwconf --example kwconf_rs_train -- \
    --config examples/parity/train.toml \
    --lr=0.01 \
    --tags=argv,override
```

or combine environment and argv:

```bash
TRAIN_TAGS=nightly,smoke \
    cargo run -p kwconf --example kwconf_rs_train -- --mode=safe
```

## Source precedence

For `Config`, values resolve in this order:

```text
defaults < config file < env < argv
```

For example, given:

```rust
#[derive(Debug, kwconf::Config)]
#[kwconf(special_options(config))]
struct TrainConfig {
    #[kwconf(default = 0.001, env = "TRAIN_LR")]
    lr: f64,

    #[kwconf(default = "fast", choices = ["fast", "safe"])]
    mode: String,

    #[kwconf(parser = "csv", env = "TRAIN_TAGS")]
    tags: Vec<String>,
}
```

a config file can provide a base override, environment can override that, and
argv wins last:

```bash
TRAIN_LR=0.005 train --config=train.toml --lr=0.01
```

Parsers apply to string sources such as argv and environment values. Config
files are already structured sources and are decoded directly into the field
type.

The outer config struct does not need to derive `Serialize` or `Deserialize`.
Resolution starts from `T::default()` and applies the winning value to each
typed field. Custom leaf types used with full `Config` may still need Serde
`Deserialize` support for structured decoding.

## Lightweight CLI

If you only need argv parsing, use `Cli` instead of `Config`:

```rust
#[derive(Debug, kwconf::Cli)]
#[kwconf(name = "train", about = "Train a model.")]
struct TrainArgs {
    #[kwconf(default = 0.001)]
    lr: f64,

    verbose: bool,

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

This path keeps the same typed-struct API but does not require Serde or the
config-file stack.

```toml
kwconf = { version = "0.1", default-features = false, features = ["derive"] }
```

If you want a manually constructed CLI rather than a derived struct, clap's
builder API is generally the better abstraction. The proc-macro-free kwconf
runtime exists as an implementation boundary and escape hatch, not as a second
clap builder API.

## Nested configs

Use `#[kwconf(subconfig)]` when a field is another kwconf config. Config files
use nested objects or tables; CLI flags use dotted paths.

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
cargo run -p kwconf --example nested -- \
    --config examples/nested.toml \
    --optimizer.lr=0.02
```

The same shape is available for argv-only parsing by deriving `kwconf::Cli` on
both structs.

## Modal subcommands

Use `ModalConfig` for subcommands with layered configuration:

```rust
#[derive(Debug, kwconf::ModalConfig)]
#[kwconf(name = "kwtool", about = "Modal CLI demo.")]
enum KwTool {
    #[kwconf(default, help = "Run training.")]
    Train(TrainConfig),

    #[kwconf(alias = "test", help = "Run evaluation.")]
    Eval(EvalConfig),
}

let command = KwTool::cli();
```

```bash
cargo run -p kwconf --example modal -- train --lr=0.02 --tags=cli,tag
```

A modal config file can select the command and provide its values:

```toml
command = "train"

[train]
lr = 0.01
tags = ["file", "demo"]
```

```bash
cargo run -p kwconf --example modal -- \
    --config examples/modal.toml \
    train --lr=0.02
```

For argv-only subcommands, use `ModalCli` with payload structs deriving `Cli`.
Both forms use clap's subcommand machinery underneath.

## Parsers

String sources are decoded according to the destination field type.

### `auto`

For full `Config`, `auto` is type-directed:

- `String` preserves the input spelling, so `--label=123` remains `"123"`.
- booleans accept `true`, `false`, `1`, and `0`.
- integers and floats parse as their destination types.
- `Option<T>` treats `null` and `none` as `None`.
- unit enums take the variant spelling.
- structured collection/object destinations can take JSON.
- an untyped `serde_json::Value` infers booleans, numbers, null, arrays, and
  objects.

Lightweight `Cli` uses ordinary Rust parsing for scalar values while retaining
kwconf's bool and option behavior.

### `csv`

`csv` splits on commas, trims surrounding whitespace, and parses each element
according to its destination element type.

Explicit empty fields are preserved:

```text
a,,b,  ->  ["a", "", "b", ""]
```

and an empty value is one explicit empty string for `Vec<String>`:

```text
--tags=  ->  [""]
```

Python kwconf currently drops empty CSV components after trimming. `kwconf-rs`
does not copy that behavior because an explicit delimiter denotes an explicit
field.

### `yaml`

`yaml` is available with full `Config` when the `yaml` Cargo feature is
enabled. Malformed YAML is an error, including for a `String` destination; it
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

`--help` is always available. Other runtime options are opt-in so ordinary
fields can still use those names.

```rust
#[kwconf(special_options(color, generate_completion))]
```

Full `Config` can additionally opt into config-file loading:

```rust
#[kwconf(special_options(config, color, generate_completion))]
```

Generate shell completions with the `completion` Cargo feature and an enabled
completion option:

```bash
cargo run -p kwconf --features completion --example kwconf_rs_train -- \
    --generate-completion bash > train.bash
cargo run -p kwconf --features completion --example kwconf_rs_train -- \
    --generate-completion zsh > _train
cargo run -p kwconf --features completion --example kwconf_rs_train -- \
    --generate-completion fish > train.fish
```

Without the feature, kwconf still recognizes an explicitly enabled
`--generate-completion` option and reports that the `completion` feature is
required. The direct `try_completion_script(...)` API returns the same error;
`completion_script(...)` is the infallible convenience wrapper and therefore
panics when generation is unavailable. This avoids pulling completion-generation
code into ordinary builds while keeping derive-generated APIs feature-stable.

`Cli` and `ModalCli` reject `special_options(config)` because they do not load
config files.

## Config files and environment

With the `config` feature, JSON files are supported. The default feature set
also enables TOML. YAML/YML support is opt-in with the `yaml` feature. Unknown
extensions are tried against the config formats enabled in that build, with all
parser failures reported if none succeeds.

Only environment variables declared by the schema are queried, so an unrelated
non-Unicode environment entry cannot crash argument parsing. `--config` paths
remain `PathBuf`s and may be non-UTF-8 on platforms that permit it.

## Features and dependencies

The default feature set provides the package's normal layered-config experience
without making every consumer compile every optional format/tool:

```toml
kwconf = "0.1"  # derive + config + JSON + TOML
```

Optional capabilities are explicit:

```toml
# YAML/YML config files and parser = "yaml"
kwconf = { version = "0.1", features = ["yaml"] }

# --generate-completion, try_completion_script(...), and completion_script(...)
kwconf = { version = "0.1", features = ["completion"] }

# Everything
kwconf = { version = "0.1", features = ["full"] }
```

The build tiers are:

```text
kwconf --no-default-features
    clap_builder runtime only
    no Serde
    no proc-macro dependency

kwconf --no-default-features --features derive
    + kwconf_derive
    + syn / quote / proc-macro2
    no Serde

kwconf (default features)
    + derive
    + layered config support
    + serde_core / serde_json
    + parse-only TOML
    no YAML
    no shell-completion generator

kwconf --features full
    + YAML
    + shell-completion generation
```

Kwconf depends directly on `clap_builder`; clap's derive feature is not enabled.
The Python-like struct API therefore uses one kwconf proc-macro layer rather than
stacking kwconf and clap derives. TOML is built with parsing support only, so
ordinary kwconf users do not compile TOML's writer.

The intended public surface is small:

- `Cli`, `ModalCli`
- `Config`, `ModalConfig`, `Sources` with the `config` feature
- `Error`, `Help`, `Result`
- `ColorChoice`, `CompletionShell`
- matching derive macros with the `derive` feature

Derive plumbing lives under `kwconf::__private` and is not stable API.

## Status

`0.1.0` is the first release. The minimum supported Rust version is 1.85.
See `docs/contract.md`, `docs/porting-from-kwconf.md`,
`docs/side-by-side-parity-demo.md`, `docs/release.md`, and `CHANGELOG.md` for
the detailed behavioral contract, parity notes, migration details, and release
build-cost gates.
