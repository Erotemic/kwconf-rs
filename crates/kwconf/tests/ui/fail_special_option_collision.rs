use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "special", special_options(config))]
struct Special {
    #[kwconf(default = "x")]
    config: String,
}

fn main() {}
