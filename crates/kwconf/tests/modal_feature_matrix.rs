use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_config(ext: &str, body: &str) -> PathBuf {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ext = ext.trim_start_matches('.');
    let path = std::env::temp_dir().join(format!(
        "kwconf-rs-modal-feature-matrix-{}-{count}.{ext}",
        std::process::id(),
    ));
    std::fs::write(&path, body).unwrap();
    path
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "train", about = "Train a model.")]
struct TrainCommand {
    #[kwconf(default = 0.001, env = "KW_MODAL_TRAIN_LR", help = "Learning rate.")]
    lr: f64,

    #[kwconf(parser = "csv", env = "KW_MODAL_TRAIN_TAGS", help = "Training tags.")]
    tags: Vec<String>,

    #[kwconf(default = 18, alias = "depth", help = "Network depth.")]
    model_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "eval", about = "Evaluate a model.")]
struct EvalCommand {
    #[kwconf(default = "val", choices = ["val", "test", "holdout"], help = "Dataset split.")]
    split: String,

    #[kwconf(default = 0.5, env = "KW_MODAL_EVAL_THRESHOLD", help = "Decision threshold.")]
    threshold: f64,

    #[kwconf(parser = "csv", env = "KW_MODAL_EVAL_METRICS", help = "Evaluation metrics.")]
    metrics: Vec<String>,

    #[kwconf(env = "KW_MODAL_EVAL_REPORT", help = "Structured report options.")]
    report: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, kwconf::ModalConfig)]
#[kwconf(name = "kwtool", about = "Exercise modal kwconf-rs behavior.")]
enum KwTool {
    #[kwconf(default, alias = "fit", help = "Run training.")]
    Train(TrainCommand),

    #[kwconf(name = "eval-model", alias = "test", help = "Run evaluation.")]
    Eval(EvalCommand),
}

fn parse_tool<I, T>(args: I) -> kwconf::Result<KwTool>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    KwTool::from_sources(kwconf::Sources::empty().with_args(args))
}

#[test]
fn modal_default_variant_is_used_when_no_command_or_config_selects_one() {
    let cmd = parse_tool(["kwtool"]).unwrap();
    match cmd {
        KwTool::Train(cfg) => {
            assert_eq!(cfg.lr, 0.001);
            assert_eq!(cfg.model_depth, 18);
            assert!(cfg.tags.is_empty());
        }
        KwTool::Eval(_) => panic!("expected default train variant"),
    }
}

#[test]
fn modal_cli_selects_variants_by_name_or_alias_and_passes_child_args_through() {
    let train = parse_tool(["kwtool", "fit", "--lr=0.2", "--tags=cli,tag", "--depth=34"])
        .unwrap();
    match train {
        KwTool::Train(cfg) => {
            assert_eq!(cfg.lr, 0.2);
            assert_eq!(cfg.tags, vec!["cli".to_string(), "tag".to_string()]);
            assert_eq!(cfg.model_depth, 34);
        }
        KwTool::Eval(_) => panic!("expected train variant from alias"),
    }

    let eval = parse_tool([
        "kwtool",
        "eval-model",
        "--split=test",
        "--threshold=0.8",
        "--metrics=acc,f1",
        "--report={\"emit\":true}",
    ])
    .unwrap();
    match eval {
        KwTool::Eval(cfg) => {
            assert_eq!(cfg.split, "test");
            assert_eq!(cfg.threshold, 0.8);
            assert_eq!(cfg.metrics, vec!["acc".to_string(), "f1".to_string()]);
            assert_eq!(cfg.report, json!({"emit": true}));
        }
        KwTool::Train(_) => panic!("expected eval variant"),
    }
}

#[test]
fn modal_config_command_may_use_an_alias_and_child_table_may_use_the_same_alias() {
    let path = temp_config(
        "toml",
        r#"
command = 'test'

[test]
split = 'holdout'
threshold = 0.7
metrics = ['file-acc', 'file-f1']
report = { source = 'alias-table' }
"#,
    );

    let cmd = parse_tool(["kwtool".to_string(), "--config".to_string(), path.display().to_string()]).unwrap();
    match cmd {
        KwTool::Eval(cfg) => {
            assert_eq!(cfg.split, "holdout");
            assert_eq!(cfg.threshold, 0.7);
            assert_eq!(cfg.metrics, vec!["file-acc".to_string(), "file-f1".to_string()]);
            assert_eq!(cfg.report, json!({"source": "alias-table"}));
        }
        KwTool::Train(_) => panic!("expected eval variant from command alias"),
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn modal_config_can_be_flat_for_the_selected_variant() {
    let path = temp_config(
        "yaml",
        "command: eval-model\nsplit: test\nthreshold: 0.6\nmetrics: [flat, yaml]\nreport:\n  source: flat\n",
    );

    let cmd = parse_tool(["kwtool".to_string(), "--config".to_string(), path.display().to_string()]).unwrap();
    match cmd {
        KwTool::Eval(cfg) => {
            assert_eq!(cfg.split, "test");
            assert_eq!(cfg.threshold, 0.6);
            assert_eq!(cfg.metrics, vec!["flat".to_string(), "yaml".to_string()]);
            assert_eq!(cfg.report, json!({"source": "flat"}));
        }
        KwTool::Train(_) => panic!("expected eval variant"),
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn modal_config_mode_field_is_also_accepted_as_the_selector() {
    let path = temp_config(
        "json",
        r#"{"mode":"train","lr":0.4,"tags":["json"],"model_depth":50}"#,
    );

    let cmd = parse_tool(["kwtool".to_string(), "--config".to_string(), path.display().to_string()]).unwrap();
    match cmd {
        KwTool::Train(cfg) => {
            assert_eq!(cfg.lr, 0.4);
            assert_eq!(cfg.tags, vec!["json".to_string()]);
            assert_eq!(cfg.model_depth, 50);
        }
        KwTool::Eval(_) => panic!("expected train variant"),
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn modal_precedence_keeps_cli_variant_above_file_variant_and_child_precedence_intact() {
    let path = temp_config(
        "toml",
        r#"
command = 'eval-model'

[train]
lr = 0.1
tags = ['file']
model_depth = 20

[eval-model]
split = 'test'
threshold = 0.2
"#,
    );

    let sources = kwconf::Sources::empty()
        .with_args([
            "kwtool".to_string(),
            "--config".to_string(),
            path.display().to_string(),
            "train".to_string(),
            "--lr=0.3".to_string(),
        ])
        .with_env_pair("KW_MODAL_TRAIN_TAGS", "env,tag");

    let cmd = KwTool::from_sources(sources).unwrap();
    match cmd {
        KwTool::Train(cfg) => {
            assert_eq!(cfg.lr, 0.3, "child argv wins over selected child table");
            assert_eq!(cfg.tags, vec!["env".to_string(), "tag".to_string()]);
            assert_eq!(cfg.model_depth, 20, "selected child table still contributes values");
        }
        KwTool::Eval(_) => panic!("CLI subcommand should override file command"),
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn modal_help_color_completion_and_child_help_are_available() {
    let help = KwTool::help_with_color(kwconf::ColorChoice::Never);
    assert!(help.contains("Exercise modal kwconf-rs behavior."));
    assert!(help.contains("train"));
    assert!(help.contains("eval-model"));
    assert!(help.contains("--config"));
    assert!(help.contains("--generate-completion"));

    let color_help = KwTool::help_with_color(kwconf::ColorChoice::Always);
    assert!(color_help.contains("\x1b["));

    let child_help = parse_tool(["kwtool", "eval-model", "--help"])
        .unwrap_err()
        .to_string();
    assert!(child_help.contains("--split"));
    assert!(child_help.contains("--threshold"));
    assert!(child_help.contains("--metrics"));

    let bash = KwTool::completion_script(kwconf::CompletionShell::Bash, "kwtool");
    assert!(bash.contains("train"));
    assert!(bash.contains("eval-model"));
    assert!(bash.contains("--lr"));
    assert!(bash.contains("--split"));
    assert!(bash.contains("--generate-completion"));
}

#[test]
fn modal_reports_invalid_variants_and_reserved_flag_errors() {
    let bad_variant = parse_tool(["kwtool", "predict"]).unwrap_err();
    assert!(bad_variant.to_string().contains("invalid modal variant"));

    let bad_shell = parse_tool(["kwtool", "--generate-completion=nu"]).unwrap_err();
    assert!(bad_shell.to_string().contains("invalid completion shell"));

    let missing_config = parse_tool(["kwtool", "--config"]).unwrap_err();
    assert!(missing_config.to_string().contains("missing value for --config"));
}
