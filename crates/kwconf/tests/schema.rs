//! Option namespace collisions are rejected before any parsing happens.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "leaf")]
struct Leaf {
    #[kwconf(default = 1)]
    value: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "flag-leaf")]
struct FlagLeaf {
    #[kwconf(default = true)]
    value: bool,
}

/// `--no-a.value` is both the negation of `a.value` and the leaf `no_a.value`.
/// Each struct is fine on its own, so only the runtime can see the clash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "collide")]
struct CrossLevelCollision {
    #[kwconf(subconfig)]
    a: FlagLeaf,

    #[kwconf(subconfig)]
    no_a: Leaf,
}

#[test]
fn cross_level_collisions_are_schema_errors() {
    let err = CrossLevelCollision::from_sources(kwconf::Sources::empty().with_args(["collide"]))
        .unwrap_err();
    match err {
        kwconf::Error::Schema(message) => {
            assert!(message.contains("--no-a.value"), "{message}");
        }
        other => panic!("expected schema error, got {other}"),
    }
}

#[test]
#[should_panic(expected = "invalid kwconf schema")]
fn help_panics_on_schema_errors() {
    let _ = CrossLevelCollision::help();
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "gen")]
struct Generic<T> {
    #[kwconf(default = 3)]
    count: u8,

    payload: T,
}

#[test]
fn generic_config_structs_derive_and_resolve() {
    let cfg = Generic::<String>::from_sources(kwconf::Sources::empty().with_args([
        "gen",
        "--payload=hello",
        "--count=4",
    ]))
    .unwrap();
    assert_eq!(cfg.payload, "hello");
    assert_eq!(cfg.count, 4);

    let cfg =
        Generic::<u64>::from_sources(kwconf::Sources::empty().with_args(["gen", "--payload=9"]))
            .unwrap();
    assert_eq!(cfg.payload, 9);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "renamed", crate = "::kwconf")]
struct CratePath {
    #[kwconf(default = 1)]
    value: u32,
}

#[test]
fn crate_path_attribute_is_accepted() {
    let cfg = CratePath::from_sources(kwconf::Sources::empty().with_args(["renamed"])).unwrap();
    assert_eq!(cfg.value, 1);
}

#[test]
fn error_source_is_exposed_for_io_failures() {
    use std::error::Error as _;
    let err = Leaf::from_sources(
        kwconf::Sources::empty()
            .with_args(["leaf"])
            .with_config_path("/definitely/missing/kwconf.toml"),
    )
    .unwrap_err();
    assert!(err.source().is_some(), "{err}");
}
