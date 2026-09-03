use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "outer")]
struct Outer<T> {
    #[kwconf(subconfig)]
    inner: T,
}

fn main() {}
