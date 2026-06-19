use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "demo", about = "Demo config.")]
struct DemoConfig {
    #[kwconf(default = 1, help = "Image width.")]
    width: usize,

    #[kwconf(default = "fast", choices = ["fast", "safe"])]
    mode: String,

    #[kwconf(parser = "csv", env = "DEMO_TAGS")]
    tags: Vec<String>,

    #[kwconf(default = false)]
    dry_run: bool,
}

#[test]
fn argv_overrides_defaults() {
    let cfg = DemoConfig::from_iter(["demo", "--width", "5", "--mode=safe", "--dry-run"]).unwrap();
    assert_eq!(cfg.width, 5);
    assert_eq!(cfg.mode, "safe");
    assert!(cfg.dry_run);
}

#[test]
fn env_uses_declared_parser() {
    let sources = kwconf::Sources::empty()
        .with_args(["demo"])
        .with_env_pair("DEMO_TAGS", "red, blue");
    let cfg = DemoConfig::from_sources(sources).unwrap();
    assert_eq!(cfg.tags, vec!["red".to_string(), "blue".to_string()]);
}

#[test]
fn config_file_is_below_argv() {
    let path = std::env::temp_dir().join(format!(
        "kwconf-rs-test-{}-{}.toml",
        std::process::id(),
        line!()
    ));
    std::fs::write(&path, "width = 8\nmode = 'safe'\ntags = ['file']\n").unwrap();

    let cfg = DemoConfig::from_iter([
        "demo".to_string(),
        "--config".to_string(),
        path.display().to_string(),
        "--width".to_string(),
        "9".to_string(),
    ])
    .unwrap();

    assert_eq!(cfg.width, 9);
    assert_eq!(cfg.mode, "safe");
    assert_eq!(cfg.tags, vec!["file".to_string()]);

    let _ = std::fs::remove_file(path);
}

#[test]
fn invalid_choice_errors() {
    let err = DemoConfig::from_iter(["demo", "--mode=slow"]).unwrap_err();
    assert!(err.to_string().contains("invalid value"));
}

#[test]
fn help_is_available() {
    let help = DemoConfig::help();
    assert!(help.contains("--width"));
    assert!(help.contains("--config"));
    assert!(help.contains("--color"));
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "optimizer", about = "Optimizer config.")]
struct OptimizerConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(default = "adam", choices = ["adam", "sgd"], help = "Optimizer kind.")]
    kind: String,

    #[kwconf(parser = "csv", env = "OPT_TAGS", help = "Optimizer tags.")]
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "nested-demo", about = "Nested config demo.")]
struct NestedConfig {
    #[kwconf(default = 64, help = "Image width.")]
    width: usize,

    #[kwconf(subconfig, help = "Optimizer settings.")]
    optimizer: OptimizerConfig,
}

#[test]
fn nested_subconfig_accepts_dotted_argv() {
    let cfg = NestedConfig::from_iter([
        "nested-demo",
        "--width=128",
        "--optimizer.lr=0.02",
        "--optimizer.kind=sgd",
    ])
    .unwrap();

    assert_eq!(cfg.width, 128);
    assert_eq!(cfg.optimizer.lr, 0.02);
    assert_eq!(cfg.optimizer.kind, "sgd");
}

#[test]
fn nested_subconfig_merges_file_env_and_argv() {
    let path = std::env::temp_dir().join(format!(
        "kwconf-rs-nested-test-{}-{}.toml",
        std::process::id(),
        line!()
    ));
    std::fs::write(
        &path,
        "width = 96\n[optimizer]\nlr = 0.03\nkind = 'sgd'\ntags = ['file']\n",
    )
    .unwrap();

    let cfg = NestedConfig::from_sources(
        kwconf::Sources::empty()
            .with_args([
                "nested-demo".to_string(),
                "--config".to_string(),
                path.display().to_string(),
                "--optimizer.lr=0.04".to_string(),
            ])
            .with_env_pair("OPT_TAGS", "env,tag"),
    )
    .unwrap();

    assert_eq!(cfg.width, 96);
    assert_eq!(cfg.optimizer.lr, 0.04);
    assert_eq!(cfg.optimizer.kind, "sgd");
    assert_eq!(cfg.optimizer.tags, vec!["env".to_string(), "tag".to_string()]);

    let _ = std::fs::remove_file(path);
}

#[test]
fn nested_help_and_completion_include_dotted_fields() {
    let help = NestedConfig::help_with_color(kwconf::ColorChoice::Never);
    assert!(help.contains("--optimizer.lr"));
    assert!(help.contains("--optimizer.kind"));

    let bash = NestedConfig::completion_script(kwconf::CompletionShell::Bash, "nested-demo");
    assert!(bash.contains("--optimizer.lr"));
    assert!(bash.contains("--optimizer.kind"));
}
