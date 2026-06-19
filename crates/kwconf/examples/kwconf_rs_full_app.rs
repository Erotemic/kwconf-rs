use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "dataset", about = "Dataset settings.")]
struct DatasetConfig {
    #[kwconf(default = "data/images", help = "Dataset root directory.")]
    root: String,

    #[kwconf(default = "train", choices = ["train", "val", "test"], help = "Dataset split.")]
    split: String,

    #[kwconf(default = vec!["red".to_string(), "green".to_string(), "blue".to_string()], parser = "csv", help = "Channel names.")]
    channels: Vec<String>,

    #[kwconf(default = false, help = "Cache decoded samples.")]
    cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "model", about = "Model settings.")]
struct ModelConfig {
    #[kwconf(default = "resnet", choices = ["resnet", "unet"], help = "Model architecture.")]
    arch: String,

    #[kwconf(default = 50, help = "Model depth.")]
    depth: usize,

    #[kwconf(default = true, help = "Use pretrained weights.")]
    pretrained: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "optimizer", about = "Optimizer settings.")]
struct OptimizerConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(default = 0.0, help = "Weight decay.")]
    weight_decay: f64,

    #[kwconf(default = "none", choices = ["none", "cosine", "step"], help = "Scheduler kind.")]
    scheduler: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "logging", about = "Logging settings.")]
struct LoggingConfig {
    #[kwconf(default = "INFO", choices = ["DEBUG", "INFO", "WARNING"], help = "Log level.")]
    level: String,

    #[kwconf(default, parser = "csv", help = "Run tags.")]
    tags: Vec<String>,

    #[kwconf(default, parser = "yaml", help = "Free-form YAML metadata.")]
    metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "train", about = "Train with nested configs and parser-aware string inputs.")]
struct TrainConfig {
    #[kwconf(default = "local", alias = "preset", choices = ["local", "debug", "cluster"], help = "Execution profile.")]
    profile: String,

    #[kwconf(subconfig, help = "Dataset settings.")]
    dataset: DatasetConfig,

    #[kwconf(subconfig, help = "Model settings.")]
    model: ModelConfig,

    #[kwconf(subconfig, help = "Optimizer settings.")]
    optimizer: OptimizerConfig,

    #[kwconf(subconfig, help = "Logging settings.")]
    logging: LoggingConfig,

    #[kwconf(default = false, alias = "dry", help = "Print the plan without running.")]
    dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "eval", about = "Evaluate a checkpoint with nested dataset settings.")]
struct EvalConfig {
    #[kwconf(default = "runs/latest/checkpoint.pt", help = "Checkpoint path.")]
    checkpoint: String,

    #[kwconf(subconfig, help = "Dataset settings.")]
    dataset: DatasetConfig,

    #[kwconf(default = vec!["accuracy".to_string()], parser = "csv", help = "Metric names.")]
    metrics: Vec<String>,

    #[kwconf(default = 0.5, help = "Decision threshold.")]
    threshold: f64,

    #[kwconf(default = true, help = "Write an evaluation report.")]
    report: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "export", about = "Export a checkpoint to an interchange format.")]
struct ExportConfig {
    #[kwconf(default = "runs/latest/checkpoint.pt", help = "Checkpoint path.")]
    checkpoint: String,

    #[kwconf(default = "exported/model.onnx", help = "Output path.")]
    output: String,

    #[kwconf(default = "onnx", choices = ["onnx", "torchscript"], help = "Export format.")]
    format: String,

    #[kwconf(default = 17, help = "ONNX opset.")]
    opset: usize,

    #[kwconf(default = true, help = "Simplify the exported graph.")]
    simplify: bool,
}

#[derive(Debug, Clone, kwconf::ModalConfig)]
#[kwconf(name = "kwconf-parity", about = "Full kwconf-rs parity demo with modal commands and nested configs.")]
enum KwconfParityApp {
    #[kwconf(default, alias = "fit", help = "Train a model.")]
    Train(TrainConfig),

    #[kwconf(alias = "score", help = "Evaluate a checkpoint.")]
    Eval(EvalConfig),

    #[kwconf(help = "Export a checkpoint.")]
    Export(ExportConfig),
}

fn emit(command: &str, config: serde_json::Value) {
    let payload = json!({
        "command": command,
        "config": config,
    });
    println!("{}", serde_json::to_string_pretty(&payload).unwrap());
}

fn main() {
    match KwconfParityApp::cli() {
        KwconfParityApp::Train(cfg) => emit("train", serde_json::to_value(cfg).unwrap()),
        KwconfParityApp::Eval(cfg) => emit("eval", serde_json::to_value(cfg).unwrap()),
        KwconfParityApp::Export(cfg) => emit("export", serde_json::to_value(cfg).unwrap()),
    }
}
