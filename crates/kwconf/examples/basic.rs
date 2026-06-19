use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "basic", about = "Small kwconf-rs demo.")]
struct BasicConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(default = "fast", choices = ["fast", "safe"], help = "Run mode.")]
    mode: String,

    #[kwconf(parser = "csv", env = "BASIC_TAGS", help = "Comma-separated tags.")]
    tags: Vec<String>,

    #[kwconf(default = true, help = "Enable cache.")]
    cache: bool,
}

fn main() {
    let cfg = BasicConfig::cli();
    println!("{cfg:#?}");
}
