use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "optimizer", about = "Optimizer settings.")]
struct OptimizerConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(default = "adam", choices = ["adam", "sgd"], help = "Optimizer kind.")]
    kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "nested", about = "Nested config demo.")]
struct JobConfig {
    #[kwconf(default = 64, help = "Image width.")]
    width: usize,

    #[kwconf(subconfig, help = "Optimizer settings.")]
    optimizer: OptimizerConfig,
}

fn main() {
    let cfg = JobConfig::cli();
    println!("{}", serde_json::to_string_pretty(&cfg).unwrap());
}
