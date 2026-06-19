use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

fn parity_train_toml() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/parity/train.toml");
    assert!(
        path.exists(),
        "missing parity fixture: {}",
        path.display()
    );
    path.display().to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "train", about = "Train a model.")]
struct TrainConfig {
    #[kwconf(default = 0.001, help = "Learning rate.")]
    lr: f64,

    #[kwconf(default = "fast", choices = ["fast", "safe"], help = "Run mode.")]
    mode: String,

    #[kwconf(parser = "csv", env = "TRAIN_TAGS", help = "Comma-separated tags.")]
    tags: Vec<String>,

    #[kwconf(default = true, help = "Enable cache.")]
    cache: bool,

    #[kwconf(default = 128, help = "Image width.")]
    width: usize,
}

#[test]
fn rust_demo_matches_kwconf_demo_contract() {
    let cfg = TrainConfig::from_iter([
        "train".to_string(),
        "--config".to_string(),
        parity_train_toml(),
        "--lr=0.01".to_string(),
        "--tags=argv,override".to_string(),
    ])
    .unwrap();

    assert_eq!(
        serde_json::to_value(cfg).unwrap(),
        json!({
            "lr": 0.01,
            "mode": "safe",
            "tags": ["argv", "override"],
            "cache": false,
            "width": 256,
        })
    );
}

#[test]
fn env_sits_between_config_and_argv() {
    let sources = kwconf::Sources::empty()
        .with_args([
            "train".to_string(),
            "--config".to_string(),
            parity_train_toml(),
            "--width=512".to_string(),
        ])
        .with_env_pair("TRAIN_TAGS", "env,tag");

    let cfg = TrainConfig::from_sources(sources).unwrap();
    assert_eq!(cfg.mode, "safe");
    assert_eq!(cfg.tags, vec!["env".to_string(), "tag".to_string()]);
    assert_eq!(cfg.width, 512);
}

#[test]
fn generated_help_and_completion_cover_demo_fields() {
    let help = TrainConfig::help_with_color(kwconf::ColorChoice::Never);
    assert!(help.contains("--generate-completion"));
    assert!(help.contains("--lr"));
    assert!(help.contains("--tags"));

    let bash = TrainConfig::completion_script(kwconf::CompletionShell::Bash, "train");
    assert!(bash.contains("--lr"));
    assert!(bash.contains("--tags"));
    assert!(bash.contains("--generate-completion"));
}

#[test]
fn cli_completion_request_returns_script() {
    let err = TrainConfig::from_iter(["train", "--generate-completion", "bash"]).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("--lr"));
    assert!(text.contains("--tags"));
}
