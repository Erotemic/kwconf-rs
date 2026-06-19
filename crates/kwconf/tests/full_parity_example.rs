use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "dataset")]
struct DatasetConfig {
    #[kwconf(default = "data/images")]
    root: String,

    #[kwconf(default = "train", choices = ["train", "val", "test"])]
    split: String,

    #[kwconf(default = vec!["red".to_string(), "green".to_string(), "blue".to_string()], parser = "csv")]
    channels: Vec<String>,

    #[kwconf(default = false)]
    cache: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "model")]
struct ModelConfig {
    #[kwconf(default = "resnet", choices = ["resnet", "unet"])]
    arch: String,

    #[kwconf(default = 50)]
    depth: usize,

    #[kwconf(default = true)]
    pretrained: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "optimizer")]
struct OptimizerConfig {
    #[kwconf(default = 0.001)]
    lr: f64,

    #[kwconf(default = 0.0)]
    weight_decay: f64,

    #[kwconf(default = "none", choices = ["none", "cosine", "step"])]
    scheduler: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "logging")]
struct LoggingConfig {
    #[kwconf(default = "INFO", choices = ["DEBUG", "INFO", "WARNING"])]
    level: String,

    #[kwconf(default, parser = "csv")]
    tags: Vec<String>,

    #[kwconf(default, parser = "yaml")]
    metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "train")]
struct TrainConfig {
    #[kwconf(default = "local", alias = "preset", choices = ["local", "debug", "cluster"])]
    profile: String,

    #[kwconf(subconfig)]
    dataset: DatasetConfig,

    #[kwconf(subconfig)]
    model: ModelConfig,

    #[kwconf(subconfig)]
    optimizer: OptimizerConfig,

    #[kwconf(subconfig)]
    logging: LoggingConfig,

    #[kwconf(default = false, alias = "dry")]
    dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "eval")]
struct EvalConfig {
    #[kwconf(default = "runs/latest/checkpoint.pt")]
    checkpoint: String,

    #[kwconf(subconfig)]
    dataset: DatasetConfig,

    #[kwconf(default = vec!["accuracy".to_string()], parser = "csv")]
    metrics: Vec<String>,

    #[kwconf(default = 0.5)]
    threshold: f64,

    #[kwconf(default = true)]
    report: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "export")]
struct ExportConfig {
    #[kwconf(default = "runs/latest/checkpoint.pt")]
    checkpoint: String,

    #[kwconf(default = "exported/model.onnx")]
    output: String,

    #[kwconf(default = "onnx", choices = ["onnx", "torchscript"])]
    format: String,

    #[kwconf(default = 17)]
    opset: usize,

    #[kwconf(default = true)]
    simplify: bool,
}

#[derive(Debug, Clone, PartialEq, kwconf::ModalConfig)]
#[kwconf(name = "kwconf-parity")]
enum KwconfParityApp {
    #[kwconf(default, alias = "fit")]
    Train(TrainConfig),

    #[kwconf(alias = "score")]
    Eval(EvalConfig),

    Export(ExportConfig),
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/parity_full").join(name)
}

#[test]
fn full_train_demo_uses_shared_yaml_and_nested_cli_overrides() {
    let train_yaml = fixture("train.yaml");
    let sources = kwconf::Sources::empty().with_args([
        "kwconf-parity".to_string(),
        "train".to_string(),
        "--config".to_string(),
        train_yaml.display().to_string(),
        "--optimizer.lr=0.02".to_string(),
        "--logging.tags=cli,side".to_string(),
        "--logging.metadata={owner: cli, priority: 2}".to_string(),
        "--dry-run".to_string(),
    ]);

    let command = KwconfParityApp::from_sources(sources).unwrap();
    match command {
        KwconfParityApp::Train(cfg) => {
            assert_eq!(cfg.profile, "debug");
            assert_eq!(cfg.dataset.root, "data/train-images");
            assert_eq!(cfg.dataset.cache, true);
            assert_eq!(cfg.optimizer.lr, 0.02);
            assert_eq!(cfg.logging.tags, vec!["cli".to_string(), "side".to_string()]);
            assert_eq!(cfg.logging.metadata.get("owner").unwrap(), &serde_json::json!("cli"));
            assert_eq!(cfg.logging.metadata.get("priority").unwrap(), &serde_json::json!(2));
            assert_eq!(cfg.dry_run, true);
        }
        other => panic!("expected train variant, got {other:?}"),
    }
}

#[test]
fn full_demo_modal_alias_and_eval_config_work() {
    let eval_yaml = fixture("eval.yaml");
    let sources = kwconf::Sources::empty().with_args([
        "kwconf-parity".to_string(),
        "score".to_string(),
        "--config".to_string(),
        eval_yaml.display().to_string(),
        "--threshold=0.91".to_string(),
        "--metrics=accuracy,f1,auc".to_string(),
    ]);

    let command = KwconfParityApp::from_sources(sources).unwrap();
    match command {
        KwconfParityApp::Eval(cfg) => {
            assert_eq!(cfg.checkpoint, "runs/debug/checkpoint.pt");
            assert_eq!(cfg.dataset.split, "val");
            assert_eq!(cfg.metrics, vec!["accuracy".to_string(), "f1".to_string(), "auc".to_string()]);
            assert_eq!(cfg.threshold, 0.91);
        }
        other => panic!("expected eval variant, got {other:?}"),
    }
}

#[test]
fn full_demo_export_config_and_override_work() {
    let export_yaml = fixture("export.yaml");
    let sources = kwconf::Sources::empty().with_args([
        "kwconf-parity".to_string(),
        "export".to_string(),
        "--config".to_string(),
        export_yaml.display().to_string(),
        "--opset=18".to_string(),
    ]);

    let command = KwconfParityApp::from_sources(sources).unwrap();
    match command {
        KwconfParityApp::Export(cfg) => {
            assert_eq!(cfg.checkpoint, "runs/debug/checkpoint.pt");
            assert_eq!(cfg.output, "exported/debug.onnx");
            assert_eq!(cfg.format, "onnx");
            assert_eq!(cfg.opset, 18);
        }
        other => panic!("expected export variant, got {other:?}"),
    }
}

#[test]
fn full_demo_help_mentions_modal_aliases_nested_fields_and_parsers() {
    let help = KwconfParityApp::help_with_color(kwconf::ColorChoice::Never);
    assert!(help.contains("train"));
    assert!(help.contains("fit"));
    assert!(help.contains("eval"));
    assert!(help.contains("score"));
    assert!(help.contains("export"));

    let child_help = KwconfParityApp::from_iter(["kwconf-parity", "train", "--help"])
        .unwrap_err()
        .to_string();
    assert!(child_help.contains("--dataset.root"));
    assert!(child_help.contains("--optimizer.lr"));
    assert!(child_help.contains("--logging.tags"));
    assert!(child_help.contains("--logging.metadata"));
    assert!(child_help.contains("parser=csv"));
    assert!(child_help.contains("parser=yaml"));
}
