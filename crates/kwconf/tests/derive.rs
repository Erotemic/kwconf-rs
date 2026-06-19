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
