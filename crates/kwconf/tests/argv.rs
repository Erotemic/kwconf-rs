//! clap owns argv recognition: help, completion, and runtime parsing agree.

use serde::{Deserialize, Serialize};
use std::ffi::OsString;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "inner")]
struct Inner {
    #[kwconf(default = 1, alias = "lr")]
    learning_rate: u32,

    #[kwconf(default = true)]
    cache: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "outer", special_options(config, color, generate_completion))]
struct Outer {
    #[kwconf(subconfig, alias = "opt")]
    optimizer: Inner,

    #[kwconf(default = "a", alias = "n")]
    long_name: String,
}

fn parse<const N: usize>(args: [&str; N]) -> kwconf::Result<Outer> {
    Outer::from_sources(kwconf::Sources::empty().with_args(args))
}

#[test]
fn aliases_apply_to_every_path_component() {
    let cfg = parse(["outer", "--opt.lr=5"]).unwrap();
    assert_eq!(cfg.optimizer.learning_rate, 5);
    let cfg = parse(["outer", "--opt.learning_rate=6"]).unwrap();
    assert_eq!(cfg.optimizer.learning_rate, 6);
    let cfg = parse(["outer", "--optimizer.lr=7"]).unwrap();
    assert_eq!(cfg.optimizer.learning_rate, 7);
    let cfg = parse(["outer", "--no-opt.cache"]).unwrap();
    assert!(!cfg.optimizer.cache);

    let help = Outer::help_with_color(kwconf::ColorChoice::Never);
    assert!(help.contains("--optimizer.learning-rate"), "{help}");
    assert!(help.contains("--optimizer.lr"), "{help}");
    assert!(help.contains("--no-optimizer.cache"), "{help}");
    assert!(
        !help.contains("--opt.lr"),
        "parent aliases are accepted but not advertised:\n{help}"
    );
}

#[test]
fn dashes_and_underscores_are_interchangeable_in_any_mix() {
    for arg in [
        "--long-name=x",
        "--long_name=x",
        "--optimizer.learning_rate=1",
        "--optimizer.learning-rate=1",
        "--opt.learning-rate=1",
        "--no-optimizer.cache",
        "--no_optimizer.cache",
    ] {
        parse(["outer", arg]).unwrap_or_else(|err| panic!("{arg}: {err}"));
    }
    assert_eq!(parse(["outer", "--long_name", "y"]).unwrap().long_name, "y");
}

#[test]
fn values_are_applied_in_argv_order_across_options() {
    let cfg = parse(["outer", "--n=1", "--long-name=2", "--n=3"]).unwrap();
    assert_eq!(cfg.long_name, "3");
    let cfg = parse([
        "outer",
        "--optimizer.cache=false",
        "--opt.cache",
        "--no-optimizer.cache",
    ])
    .unwrap();
    assert!(!cfg.optimizer.cache);
}

#[test]
fn unknown_arguments_and_trailing_positionals_are_rejected() {
    let err = parse(["outer", "--long-nam=1"]).unwrap_err();
    let text = err.to_string();
    assert!(text.starts_with("unknown argument: --long-nam"), "{text}");
    assert!(text.contains("did you mean --long-name"), "{text}");

    let err = parse(["outer", "stray"]).unwrap_err();
    assert!(matches!(err, kwconf::Error::UnknownArgument(_)), "{err}");

    let err = parse(["outer", "--", "stray"]).unwrap_err();
    assert!(matches!(err, kwconf::Error::UnknownArgument(_)), "{err}");
}

#[test]
fn short_help_and_help_after_options_work() {
    for args in [vec!["outer", "-h"], vec!["outer", "--n=1", "--help"]] {
        let err = Outer::from_sources(kwconf::Sources::empty().with_args(args)).unwrap_err();
        match err {
            kwconf::Error::HelpRequested(help) => {
                assert!(help.plain().contains("--long-name"));
                assert!(!help.plain().contains("\x1b["));
                assert!(help.ansi().contains("\x1b["));
                assert!(matches!(help.color(), kwconf::ColorChoice::Auto));
            }
            other => panic!("expected help, got {other}"),
        }
    }

    let err = parse(["outer", "--help", "--color=always"]).unwrap_err();
    assert!(
        err.to_string().contains("\x1b["),
        "color after --help still applies"
    );
}

#[test]
fn help_lists_the_negation_flag_once_and_a_bool_takes_an_optional_value() {
    let help = Outer::help_with_color(kwconf::ColorChoice::Never);
    assert_eq!(help.matches("--no-optimizer.cache").count(), 1, "{help}");
    assert!(help.contains("--optimizer.cache [<VALUE>]"), "{help}");
    let cfg = parse(["outer", "--optimizer.cache", "--long-name=z"]).unwrap();
    assert!(cfg.optimizer.cache);
    assert_eq!(cfg.long_name, "z");
}

#[test]
fn completion_scripts_come_from_the_same_model() {
    let bash = Outer::completion_script(kwconf::CompletionShell::Bash, "outer");
    assert!(bash.contains("--optimizer.learning-rate"));
    assert!(bash.contains("--no-optimizer.cache"));
    assert!(bash.contains("--generate-completion"));
}

#[test]
fn non_utf8_argv_is_only_rejected_where_it_is_used() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let bad = OsString::from_vec(b"\xff\xfe".to_vec());
        let sources = kwconf::Sources::empty().with_args([
            OsString::from("outer"),
            OsString::from("--config"),
            bad.clone(),
        ]);
        let err = Outer::from_sources(sources).unwrap_err();
        assert!(
            matches!(err, kwconf::Error::Io { .. }),
            "a non-UTF-8 config path reaches the file system: {err}"
        );

        let sources = kwconf::Sources::empty().with_args([
            OsString::from("outer"),
            OsString::from("--long-name"),
            bad,
        ]);
        let err = Outer::from_sources(sources).unwrap_err();
        assert!(!matches!(err, kwconf::Error::Io { .. }), "{err}");
    }
    let _ = OsString::new();
}
