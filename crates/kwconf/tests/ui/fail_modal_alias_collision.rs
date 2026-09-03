use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "leaf")]
struct Leaf {
    #[kwconf(default = 1)]
    value: u32,
}

#[derive(Debug, Clone, kwconf::ModalConfig)]
#[kwconf(name = "tool")]
enum Tool {
    #[kwconf(alias = "eval")]
    Train(Leaf),

    Eval(Leaf),
}

fn main() {}
