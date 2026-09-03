use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(
    name = "train",
    about = "Train a model.",
    special_options(config, color, generate_completion)
)]
struct TrainConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(parser = "csv", env = "TRAIN_TAGS", help = "Tags.")]
    tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(
    name = "eval",
    about = "Evaluate a model.",
    special_options(config, color, generate_completion)
)]
struct EvalConfig {
    #[kwconf(default = "val", help = "Dataset split.")]
    split: String,

    #[kwconf(default = 0.5, help = "Decision threshold.")]
    threshold: f64,
}

#[derive(Debug, Clone, kwconf::ModalConfig)]
#[kwconf(
    name = "kwtool",
    about = "Modal CLI demo.",
    special_options(config, color, generate_completion)
)]
enum KwTool {
    #[kwconf(default, help = "Run training.")]
    Train(TrainConfig),

    #[kwconf(alias = "test", help = "Run evaluation.")]
    Eval(EvalConfig),
}

fn main() {
    let command = KwTool::cli();
    match command {
        KwTool::Train(cfg) => println!("train {}", serde_json::to_string_pretty(&cfg).unwrap()),
        KwTool::Eval(cfg) => println!("eval {}", serde_json::to_string_pretty(&cfg).unwrap()),
    }
}
