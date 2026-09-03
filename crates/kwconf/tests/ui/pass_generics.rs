use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "generic")]
struct Generic<T>
where
    T: Clone,
{
    #[kwconf(default = 2)]
    count: u8,

    payload: T,

    items: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "renamed", crate = "::kwconf")]
struct CratePath {
    #[kwconf(default = "x")]
    value: String,
}

fn main() {
    let cfg = Generic::<String>::from_sources(
        kwconf::Sources::empty().with_args(["generic", "--payload=hi"]),
    )
    .unwrap();
    assert_eq!(cfg.payload, "hi");
    let _ = CratePath::from_sources(kwconf::Sources::empty().with_args(["renamed"])).unwrap();
}
