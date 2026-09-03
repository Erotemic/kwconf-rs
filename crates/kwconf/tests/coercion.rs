//! argv and env text is coerced by the destination type, not by its spelling.

use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    #[default]
    Fast,
    Safe,
}

#[derive(Debug, Clone, PartialEq, kwconf::Config)]
#[kwconf(name = "coerce")]
struct CoerceConfig {
    #[kwconf(env = "KWC_LABEL")]
    label: String,

    #[kwconf(env = "KWC_COUNT")]
    count: u32,

    #[kwconf(default = -1)]
    offset: i64,

    #[kwconf(default = 0.5)]
    ratio: f64,

    #[kwconf(default = false, env = "KWC_FLAG")]
    flag: bool,

    #[kwconf(env = "KWC_MAYBE")]
    maybe: Option<String>,

    #[kwconf(parser = "csv", env = "KWC_INTS")]
    ints: Vec<i64>,

    #[kwconf(parser = "csv")]
    words: Vec<String>,

    #[kwconf(default = Mode::Fast, choices = ["fast", "safe"])]
    mode: Mode,

    #[kwconf(default = 'x')]
    letter: char,

    #[kwconf(parser = "yaml")]
    table: BTreeMap<String, i64>,

    #[kwconf(parser = "yaml")]
    yaml_text: String,

    any: serde_json::Value,
}

fn parse<const N: usize>(args: [&str; N]) -> kwconf::Result<CoerceConfig> {
    CoerceConfig::from_sources(kwconf::Sources::empty().with_args(args))
}

#[test]
fn string_fields_keep_values_that_look_like_other_types() {
    for text in [
        "true",
        "123",
        "null",
        "none",
        "[1,2]",
        "{\"a\":1}",
        "1.5",
        " padded ",
    ] {
        let cfg = parse(["coerce", &format!("--label={text}")]).unwrap();
        assert_eq!(cfg.label, text, "argv {text:?}");

        let cfg = CoerceConfig::from_sources(
            kwconf::Sources::empty()
                .with_args(["coerce"])
                .with_env_pair("KWC_LABEL", text),
        )
        .unwrap();
        assert_eq!(cfg.label, text, "env {text:?}");
    }
}

#[test]
fn numeric_fields_parse_by_type_and_report_the_field() {
    let cfg = parse(["coerce", "--count=42", "--offset=-7", "--ratio=1e-3"]).unwrap();
    assert_eq!(cfg.count, 42);
    assert_eq!(cfg.offset, -7);
    assert_eq!(cfg.ratio, 0.001);

    let err = parse(["coerce", "--count=-1"]).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("invalid value for count"), "{text}");
    assert!(text.contains("unsigned integer"), "{text}");

    let err = parse(["coerce", "--count=4294967296"]).unwrap_err();
    assert!(err.to_string().contains("count"), "{err}");

    let err = parse(["coerce", "--ratio=fast"]).unwrap_err();
    assert!(err.to_string().contains("invalid value for ratio"), "{err}");
}

#[test]
fn bool_fields_accept_strict_kwconf_spellings_and_negation() {
    for (text, expected) in [
        ("true", true),
        ("false", false),
        ("1", true),
        ("0", false),
        ("TRUE", true),
    ] {
        let cfg = parse(["coerce", &format!("--flag={text}")]).unwrap();
        assert_eq!(cfg.flag, expected, "{text:?}");
    }
    assert!(parse(["coerce", "--flag"]).unwrap().flag);
    assert!(!parse(["coerce", "--no-flag"]).unwrap().flag);
    assert!(!parse(["coerce", "--flag", "--no-flag"]).unwrap().flag);
    assert!(parse(["coerce", "--no-flag", "--flag"]).unwrap().flag);
    assert!(
        parse(["coerce", "--no-flag", "--no-flag", "--flag=true"])
            .unwrap()
            .flag
    );
    assert!(
        !parse(["coerce", "--flag", "--no-flag", "--no-flag"])
            .unwrap()
            .flag
    );

    for invalid in ["yes", "no", "on", "off", "maybe"] {
        let err = parse(["coerce", &format!("--flag={invalid}")]).unwrap_err();
        assert!(err.to_string().contains("invalid value for flag"), "{err}");
    }

    let cfg = CoerceConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["coerce"])
            .with_env_pair("KWC_FLAG", "1"),
    )
    .unwrap();
    assert!(cfg.flag);
}

#[test]
fn option_fields_treat_null_and_none_as_absent() {
    assert_eq!(parse(["coerce", "--maybe=null"]).unwrap().maybe, None);
    assert_eq!(parse(["coerce", "--maybe=None"]).unwrap().maybe, None);
    assert_eq!(
        parse(["coerce", "--maybe=123"]).unwrap().maybe,
        Some("123".to_string())
    );
    assert_eq!(
        parse(["coerce", "--maybe="]).unwrap().maybe,
        Some(String::new())
    );
}

#[test]
fn csv_elements_are_coerced_by_the_element_type() {
    let cfg = parse(["coerce", "--ints=1, 2,3", "--words=1, 2,3"]).unwrap();
    assert_eq!(cfg.ints, vec![1, 2, 3]);
    assert_eq!(
        cfg.words,
        vec!["1".to_string(), "2".to_string(), "3".to_string()]
    );
    let cfg = parse(["coerce", "--words=a,,b,"]).unwrap();
    assert_eq!(cfg.words, vec!["a", "", "b", ""]);
    let cfg = parse(["coerce", "--words="]).unwrap();
    assert_eq!(cfg.words, vec![""]);
    assert!(parse(["coerce", "--ints="]).is_err());

    let err = parse(["coerce", "--ints=1,x"]).unwrap_err();
    assert!(err.to_string().contains("invalid value for ints"), "{err}");

    let cfg = CoerceConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["coerce"])
            .with_env_pair("KWC_INTS", "4,5"),
    )
    .unwrap();
    assert_eq!(cfg.ints, vec![4, 5]);
}

#[test]
fn unit_enums_parse_from_bare_strings_and_respect_choices() {
    assert_eq!(parse(["coerce", "--mode=safe"]).unwrap().mode, Mode::Safe);
    let err = parse(["coerce", "--mode=slow"]).unwrap_err();
    assert!(matches!(err, kwconf::Error::Choice { .. }), "{err}");

    let cfg = CoerceConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["coerce"])
            .with_config_value(json!({"mode": "safe"})),
    )
    .unwrap();
    assert_eq!(cfg.mode, Mode::Safe);
}

#[cfg(feature = "yaml")]
#[test]
fn chars_yaml_and_any_values() {
    let cfg = parse([
        "coerce",
        "--letter=q",
        "--table={a: 1, b: 2}",
        "--yaml-text=123",
        "--any=123",
    ])
    .unwrap();
    assert_eq!(cfg.letter, 'q');
    assert_eq!(
        cfg.table,
        BTreeMap::from([("a".to_string(), 1), ("b".to_string(), 2)])
    );
    assert_eq!(
        cfg.yaml_text, "123",
        "yaml scalars stay text for String fields"
    );
    assert_eq!(cfg.any, json!(123), "auto inference still applies to Value");

    let err = parse(["coerce", "--letter=qq"]).unwrap_err();
    assert!(
        err.to_string().contains("invalid value for letter"),
        "{err}"
    );

    let cfg = parse(["coerce", "--yaml-text=\"quoted\""]).unwrap();
    assert_eq!(cfg.yaml_text, "quoted");
}

#[test]
fn deserialize_errors_name_nested_fields() {
    #[derive(Debug, Clone, PartialEq, kwconf::Config)]
    #[kwconf(name = "inner")]
    struct Inner {
        #[kwconf(env = "KWC_INNER_N")]
        n: u8,
    }

    #[derive(Debug, Clone, PartialEq, kwconf::Config)]
    #[kwconf(name = "outer")]
    struct Outer {
        #[kwconf(subconfig)]
        inner: Inner,
    }

    let err = Outer::from_sources(kwconf::Sources::empty().with_args(["outer", "--inner.n=300"]))
        .unwrap_err();
    match err {
        kwconf::Error::InvalidValue { field, message } => {
            assert_eq!(field, "inner.n");
            assert!(message.contains("300"), "{message}");
        }
        other => panic!("expected invalid-value error, got {other}"),
    }

    let err = Outer::from_sources(
        kwconf::Sources::empty()
            .with_args(["outer"])
            .with_env_pair("KWC_INNER_N", "x"),
    )
    .unwrap_err();
    assert!(err.to_string().contains("inner.n"), "{err}");
    assert!(err.to_string().contains("(env)"), "{err}");
}

#[test]
fn outer_serde_field_names_do_not_shape_kwconf() {
    #[derive(Debug, Clone, PartialEq, Deserialize, kwconf::Config)]
    #[kwconf(name = "serde-shape")]
    struct SerdeShape {
        #[serde(rename = "wire-name")]
        #[kwconf(default = 1)]
        rust_name: u8,
    }

    let cfg = SerdeShape::from_sources(
        kwconf::Sources::empty()
            .with_args(["serde-shape"])
            .with_config_value(json!({"rust_name": 9})),
    )
    .unwrap();
    assert_eq!(cfg.rust_name, 9);

    let err = SerdeShape::from_sources(
        kwconf::Sources::empty()
            .with_args(["serde-shape"])
            .with_config_value(json!({"wire-name": 9})),
    )
    .unwrap_err();
    assert!(matches!(err, kwconf::Error::UnknownField { .. }), "{err}");
}
