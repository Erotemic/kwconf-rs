use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "dup")]
struct Dup {
    #[kwconf(default = 1, alias = "learning-rate")]
    lr: u32,

    #[kwconf(default = 2)]
    learning_rate: u32,
}

fn main() {}
