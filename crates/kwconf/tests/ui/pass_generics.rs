#[derive(Debug, Clone, kwconf::Config)]
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

#[derive(Debug, Clone, kwconf::Cli)]
#[kwconf(name = "generic-cli")]
struct GenericCli<T>
where
    T: Clone,
{
    payload: T,
}

#[derive(Debug, Clone, kwconf::Config)]
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

    let cli = GenericCli::<u16>::from_iter(["generic-cli", "--payload=7"]).unwrap();
    assert_eq!(cli.payload, 7);

    let _ = CratePath::from_sources(kwconf::Sources::empty().with_args(["renamed"])).unwrap();
}
