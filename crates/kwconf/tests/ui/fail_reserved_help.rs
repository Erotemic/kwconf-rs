use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "reserved")]
struct Reserved {
    #[kwconf(default = "x")]
    help: String,
}

fn main() {}
