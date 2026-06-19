# Side-by-side kwconf / kwconf-rs parity demo

This repo includes a full demo in both languages so you can inspect and run the
same CLI shape yourself.

- Python: `examples/parity_full/kwconf_app.py`
- Rust: `crates/kwconf/examples/kwconf_rs_full_app.rs`
- Shared YAML fixtures: `examples/parity_full/*.yaml`

The demo covers the features that matter most when porting a kwconf CLI to Rust:

- modal commands: `train`, `eval`, and `export`;
- modal aliases: `fit` for `train`, `score` for `eval`;
- nested subconfigs: `dataset`, `model`, `optimizer`, and `logging`;
- dotted CLI overrides such as `--optimizer.lr=0.02`;
- field aliases such as `--preset=debug`;
- string parsers: csv for tags and metrics, yaml for metadata;
- string choices for profile, split, architecture, scheduler, log level, and export format;
- boolean flags such as `--dry-run`;
- config-file values overridden by argv.

## Build the Rust example

From the repo root:

```bash
cargo build -p kwconf --example kwconf_rs_full_app
```

Run the Rust test suite, including tests that exercise the same fixtures used by
this demo:

```bash
cargo test -p kwconf --test full_parity_example
```

## Prepare the Python example

The Python example expects Python `kwconf` to be importable. Use an installed
package:

```bash
python -m pip install kwconf
```

Or point at a sibling checkout while developing both repos:

```bash
export PYTHONPATH="$HOME/code/kwconf:${PYTHONPATH}"
```

## Compare top-level help

```bash
python examples/parity_full/kwconf_app.py --help
cargo run -p kwconf --example kwconf_rs_full_app -- --help
```

## Compare train help

```bash
python examples/parity_full/kwconf_app.py train --help
cargo run -p kwconf --example kwconf_rs_full_app -- train --help
```

Look for the same conceptual surfaces: nested `dataset.*`, `model.*`,
`optimizer.*`, `logging.*` flags, parser hints for csv/yaml fields, and modal
aliases.

## Run train side by side

Both commands use the same YAML file and then override several values from argv.

```bash
python examples/parity_full/kwconf_app.py train \
    --config examples/parity_full/train.yaml \
    --preset=debug \
    --optimizer.lr=0.02 \
    --logging.tags=cli,side \
    --logging.metadata='{owner: cli, priority: 2}' \
    --dry-run
```

```bash
cargo run -p kwconf --example kwconf_rs_full_app -- train \
    --config examples/parity_full/train.yaml \
    --preset=debug \
    --optimizer.lr=0.02 \
    --logging.tags=cli,side \
    --logging.metadata='{owner: cli, priority: 2}' \
    --dry-run
```

Expected shape:

```json
{
  "command": "train",
  "config": {
    "dataset": {
      "cache": true,
      "channels": ["red", "green", "blue"],
      "root": "data/train-images",
      "split": "train"
    },
    "dry_run": true,
    "logging": {
      "level": "INFO",
      "metadata": {
        "owner": "cli",
        "priority": 2
      },
      "tags": ["cli", "side"]
    },
    "model": {
      "arch": "resnet",
      "depth": 50,
      "pretrained": true
    },
    "optimizer": {
      "lr": 0.02,
      "scheduler": "cosine",
      "weight_decay": 0.0001
    },
    "profile": "debug"
  }
}
```

## Run eval via the alias

```bash
python examples/parity_full/kwconf_app.py score \
    --config examples/parity_full/eval.yaml \
    --threshold=0.91 \
    --metrics=accuracy,f1,auc
```

```bash
cargo run -p kwconf --example kwconf_rs_full_app -- score \
    --config examples/parity_full/eval.yaml \
    --threshold=0.91 \
    --metrics=accuracy,f1,auc
```

## Run export

```bash
python examples/parity_full/kwconf_app.py export \
    --config examples/parity_full/export.yaml \
    --opset=18
```

```bash
cargo run -p kwconf --example kwconf_rs_full_app -- export \
    --config examples/parity_full/export.yaml \
    --opset=18
```

## Generate completions for the Rust example

Python kwconf can delegate autocomplete to `argcomplete` when it is installed.
The Rust example exposes completion generation through clap-compatible scripts:

```bash
cargo run -p kwconf --example kwconf_rs_full_app -- --generate-completion bash > kwconf-parity.bash
cargo run -p kwconf --example kwconf_rs_full_app -- --generate-completion zsh > _kwconf-parity
cargo run -p kwconf --example kwconf_rs_full_app -- --generate-completion fish > kwconf-parity.fish
```

## Force colored Rust help

```bash
cargo run -p kwconf --example kwconf_rs_full_app -- --color always --help
cargo run -p kwconf --example kwconf_rs_full_app -- train --color always --help
```

The Python side uses rich argparse formatting when the optional Python extras are
installed and enabled. The Rust side uses clap styling and `--color` to make the
behavior explicit and testable.
