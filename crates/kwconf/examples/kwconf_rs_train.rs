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

    #[kwconf(default = true, help = "Enable cache.")]
    cache: bool,

    #[kwconf(default = 128, help = "Image width.")]
    width: usize,
}

fn main() {
    let cfg = TrainConfig::cli();
    println!("{}", serde_json::to_string_pretty(&cfg).unwrap());
}
