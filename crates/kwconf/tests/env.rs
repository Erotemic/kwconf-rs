//! Only declared env bindings are read, and only when asked.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "envdemo")]
struct EnvConfig {
    #[kwconf(default = "unset", env = "KWCONF_TEST_LABEL_7f3a")]
    label: String,
}

#[test]
fn empty_sources_ignore_the_process_environment() {
    std::env::set_var("KWCONF_TEST_LABEL_7f3a", "from-process");
    let cfg = EnvConfig::from_sources(kwconf::Sources::empty().with_args(["envdemo"])).unwrap();
    assert_eq!(cfg.label, "unset");

    let cfg = EnvConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["envdemo"])
            .with_process_env(true),
    )
    .unwrap();
    assert_eq!(cfg.label, "from-process");

    let cfg = EnvConfig::from_sources(
        kwconf::Sources::from_iter(["envdemo"]).with_env_pair("KWCONF_TEST_LABEL_7f3a", "explicit"),
    )
    .unwrap();
    assert_eq!(
        cfg.label, "explicit",
        "explicit bindings win over the process"
    );

    let cfg = EnvConfig::from_iter(["envdemo"]).unwrap();
    assert_eq!(cfg.label, "from-process", "from_iter reads the process env");
}

#[cfg(unix)]
#[test]
fn unrelated_non_unicode_environment_variables_do_not_panic() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    std::env::set_var(
        "KWCONF_TEST_GARBAGE_7f3a",
        OsString::from_vec(b"\xff\xfe".to_vec()),
    );
    let cfg = EnvConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["envdemo"])
            .with_process_env(true),
    )
    .unwrap();
    assert!(cfg.label == "unset" || cfg.label == "from-process");

    std::env::set_var(
        "KWCONF_TEST_LABEL_BAD_7f3a",
        OsString::from_vec(b"\xff".to_vec()),
    );

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
    #[kwconf(name = "envbad")]
    struct BadEnvConfig {
        #[kwconf(env = "KWCONF_TEST_LABEL_BAD_7f3a")]
        label: String,
    }

    let err = BadEnvConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["envbad"])
            .with_process_env(true),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not valid UTF-8"), "{err}");
}
