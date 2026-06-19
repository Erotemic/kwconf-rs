use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "train", about = "Train a model.")]
struct TrainConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(parser = "csv", env = "TRAIN_TAGS", help = "Tags.")]
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "eval", about = "Evaluate a model.")]
struct EvalConfig {
    #[kwconf(default = "val", help = "Dataset split.")]
    split: String,

    #[kwconf(default = 0.5, help = "Decision threshold.")]
    threshold: f64,
}

#[derive(Debug, Clone, PartialEq, kwconf::ModalConfig)]
#[kwconf(name = "kwtool", about = "Demo modal CLI.")]
enum KwTool {
    #[kwconf(default, help = "Run training.")]
    Train(TrainConfig),

    #[kwconf(alias = "test", help = "Run evaluation.")]
    Eval(EvalConfig),
}

#[test]
fn modal_cli_selects_subcommand() {
    let cmd = KwTool::from_iter(["kwtool", "train", "--lr=0.02", "--tags=cli,tag"]).unwrap();
    match cmd {
        KwTool::Train(cfg) => {
            assert_eq!(cfg.lr, 0.02);
            assert_eq!(cfg.tags, vec!["cli".to_string(), "tag".to_string()]);
        }
        KwTool::Eval(_) => panic!("expected train variant"),
    }
}

#[test]
fn modal_config_file_selects_variant_and_child_table() {
    let path = std::env::temp_dir().join(format!(
        "kwconf-rs-modal-test-{}-{}.toml",
        std::process::id(),
        line!()
    ));
    std::fs::write(
        &path,
        "command = 'eval'\n[train]\nlr = 0.1\ntags = ['file']\n[eval]\nsplit = 'test'\nthreshold = 0.9\n",
    )
    .unwrap();

    let cmd = KwTool::from_iter(["kwtool".to_string(), "--config".to_string(), path.display().to_string()]).unwrap();
    match cmd {
        KwTool::Eval(cfg) => {
            assert_eq!(serde_json::to_value(cfg).unwrap(), json!({
                "split": "test",
                "threshold": 0.9,
            }));
        }
        KwTool::Train(_) => panic!("expected eval variant"),
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn modal_env_and_argv_override_config() {
    let path = std::env::temp_dir().join(format!(
        "kwconf-rs-modal-test-{}-{}.toml",
        std::process::id(),
        line!()
    ));
    std::fs::write(&path, "command = 'train'\n[train]\nlr = 0.1\ntags = ['file']\n").unwrap();

    let sources = kwconf::Sources::empty()
        .with_args([
            "kwtool".to_string(),
            "--config".to_string(),
            path.display().to_string(),
            "train".to_string(),
            "--lr=0.2".to_string(),
        ])
        .with_env_pair("TRAIN_TAGS", "env,tag");

    let cmd = KwTool::from_sources(sources).unwrap();
    match cmd {
        KwTool::Train(cfg) => {
            assert_eq!(cfg.lr, 0.2);
            assert_eq!(cfg.tags, vec!["env".to_string(), "tag".to_string()]);
        }
        KwTool::Eval(_) => panic!("expected train variant"),
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn modal_help_completion_and_child_help_work() {
    let help = KwTool::help_with_color(kwconf::ColorChoice::Never);
    assert!(help.contains("train"));
    assert!(help.contains("eval"));
    assert!(help.contains("--config"));

    let child_help = KwTool::from_iter(["kwtool", "train", "--help"]).unwrap_err().to_string();
    assert!(child_help.contains("--lr"));
    assert!(child_help.contains("--tags"));

    let bash = KwTool::completion_script(kwconf::CompletionShell::Bash, "kwtool");
    assert!(bash.contains("train"));
    assert!(bash.contains("eval"));
    assert!(bash.contains("--lr"));
}
