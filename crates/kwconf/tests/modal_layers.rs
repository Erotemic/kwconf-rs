//! Modal root and child config files layer, and child help names the subcommand.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_config(ext: &str, body: &str) -> PathBuf {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "kwconf-rs-modal-layers-{}-{count}.{ext}",
        std::process::id(),
    ));
    std::fs::write(&path, body).unwrap();
    path
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(
    name = "train",
    about = "Train with all the knobs.",
    special_options(config, color, generate_completion)
)]
struct TrainCommand {
    #[kwconf(default = 0.001)]
    lr: f64,

    #[kwconf(default = 10)]
    epochs: u32,

    #[kwconf(default = "cpu")]
    device: String,
}

#[derive(Debug, Clone, PartialEq, kwconf::ModalConfig)]
#[kwconf(name = "kwtool", special_options(config, color, generate_completion))]
enum KwTool {
    #[kwconf(default, help = "Run training.")]
    Train(TrainCommand),
}

#[test]
fn child_config_layers_over_the_root_child_table() {
    let root = temp_config(
        "toml",
        "command = 'train'\n[train]\nlr = 0.5\nepochs = 3\ndevice = 'root'\n",
    );
    let child = temp_config("toml", "epochs = 7\ndevice = 'child'\n");

    let cmd = KwTool::from_sources(kwconf::Sources::empty().with_args([
        "kwtool".to_string(),
        "--config".to_string(),
        root.display().to_string(),
        "train".to_string(),
        "--config".to_string(),
        child.display().to_string(),
        "--device=argv".to_string(),
    ]))
    .unwrap();
    let KwTool::Train(cfg) = cmd;
    assert_eq!(cfg.lr, 0.5, "root child table still applies");
    assert_eq!(cfg.epochs, 7, "child file wins over the root child table");
    assert_eq!(cfg.device, "argv", "argv wins over both files");

    let _ = std::fs::remove_file(root);
    let _ = std::fs::remove_file(child);
}

#[test]
fn child_help_uses_the_full_command_path_and_variant_help() {
    let err = KwTool::from_iter(["kwtool", "train", "-h"]).unwrap_err();
    let kwconf::Error::HelpRequested(help) = err else {
        panic!("expected help");
    };
    assert!(
        help.plain().contains("Usage: kwtool train"),
        "{}",
        help.plain()
    );
    assert!(
        help.plain().contains("Run training."),
        "variant help replaces the config about:
{}",
        help.plain()
    );
    assert!(help.plain().contains("--epochs"), "{}", help.plain());

    let root = KwTool::help_with_color(kwconf::ColorChoice::Never);
    assert!(root.contains("train  Run training."), "{root}");
    assert!(!root.contains("help  Print this message"), "{root}");
}

#[test]
fn root_and_child_color_flags_both_apply_to_child_help() {
    let err = KwTool::from_iter(["kwtool", "--color=always", "train", "--help"]).unwrap_err();
    assert!(err.to_string().contains("\x1b["));
    let err = KwTool::from_iter(["kwtool", "train", "--color=never", "--help"]).unwrap_err();
    assert!(!err.to_string().contains("\x1b["));
}

#[test]
fn modal_child_table_keys_are_dash_underscore_insensitive() {
    #[derive(Debug, Clone, PartialEq, kwconf::ModalConfig)]
    #[kwconf(name = "tool2", special_options(config))]
    enum Tool2 {
        #[kwconf(name = "run-model")]
        RunModel(TrainCommand),
    }

    let root = temp_config("toml", "command = 'run_model'\n[run_model]\nepochs = 42\n");
    let cmd = Tool2::from_sources(kwconf::Sources::empty().with_args([
        "tool2".to_string(),
        "--config".to_string(),
        root.display().to_string(),
    ]))
    .unwrap();
    let Tool2::RunModel(cfg) = cmd;
    assert_eq!(cfg.epochs, 42);
    let _ = std::fs::remove_file(root);
}
