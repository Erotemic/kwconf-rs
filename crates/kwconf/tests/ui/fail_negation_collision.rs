use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "neg")]
struct Neg {
    #[kwconf(default = true)]
    cache: bool,

    #[kwconf(default = 1)]
    no_cache: u32,
}

fn main() {}
