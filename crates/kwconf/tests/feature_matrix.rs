use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_config(ext: &str, body: &str) -> PathBuf {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ext = ext.trim_start_matches('.');
    let path = std::env::temp_dir().join(format!(
        "kwconf-rs-feature-matrix-{}-{count}.{ext}",
        std::process::id(),
    ));
    std::fs::write(&path, body).unwrap();
    path
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "backend", about = "Backend model options.")]
struct BackendConfig {
    #[kwconf(default = "resnet", alias = "arch", choices = ["resnet", "vit", "toy"], help = "Model architecture.")]
    architecture: String,

    #[kwconf(default = 32, alias = "bs", help = "Batch size.")]
    batch_size: usize,

    #[kwconf(parser = "csv", env = "KW_BACKEND_LABELS", help = "Backend labels.")]
    labels: Vec<String>,

    #[kwconf(parser = "yaml", env = "KW_BACKEND_AUGS", help = "Backend augmentations.")]
    augmentations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "feature-matrix", about = "Exercise supported kwconf-rs features.")]
struct FeatureMatrixConfig {
    #[kwconf(default = 0.001, env = "KW_LR", help = "Learning rate.")]
    lr: f64,

    #[kwconf(default = true, env = "KW_CACHE", help = "Use cache.")]
    cache: bool,

    #[kwconf(default = "fast", alias = "run-mode", choices = ["fast", "safe", "debug"], env = "KW_MODE", help = "Execution mode.")]
    mode: String,

    #[kwconf(parser = "csv", env = "KW_TAGS", help = "Comma-separated tags.")]
    tags: Vec<String>,

    #[kwconf(parser = "yaml", env = "KW_SCHEDULE", help = "YAML schedule.")]
    schedule: Vec<String>,

    #[kwconf(env = "KW_RETRIES", help = "Retry count.")]
    retries: usize,

    #[kwconf(env = "KW_MAYBE_LABEL", help = "Optional label.")]
    maybe_label: Option<String>,

    #[kwconf(env = "KW_NUMBERS", help = "JSON integer list parsed by auto.")]
    numbers: Vec<i64>,

    #[kwconf(env = "KW_METADATA", help = "JSON object parsed by auto.")]
    metadata: serde_json::Value,

    #[kwconf(subconfig, help = "Nested backend config.")]
    backend: BackendConfig,
}

fn parse_feature<I, T>(args: I) -> kwconf::Result<FeatureMatrixConfig>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    FeatureMatrixConfig::from_sources(kwconf::Sources::empty().with_args(args))
}

#[test]
fn defaults_are_deserializeable_and_nested_defaults_are_present() {
    let cfg = parse_feature(["feature-matrix"]).unwrap();

    assert_eq!(cfg.lr, 0.001);
    assert!(cfg.cache);
    assert_eq!(cfg.mode, "fast");
    assert!(cfg.tags.is_empty());
    assert!(cfg.schedule.is_empty());
    assert_eq!(cfg.retries, 0);
    assert_eq!(cfg.maybe_label, None);
    assert!(cfg.numbers.is_empty());
    assert_eq!(cfg.metadata, serde_json::Value::Null);
    assert_eq!(cfg.backend.architecture, "resnet");
    assert_eq!(cfg.backend.batch_size, 32);
}

#[test]
fn source_precedence_is_defaults_config_env_then_argv_for_leaf_and_nested_values() {
    let path = temp_config(
        "toml",
        r#"
lr = 0.01
cache = false
mode = 'safe'
tags = ['file']
schedule = ['file-schedule']
retries = 2
maybe_label = 'file-label'
numbers = [4, 5]
metadata = { source = 'file' }

[backend]
architecture = 'vit'
batch_size = 64
labels = ['file-label']
augmentations = ['file-aug']
"#,
    );

    let sources = kwconf::Sources::empty()
        .with_args([
            "feature-matrix".to_string(),
            "--config".to_string(),
            path.display().to_string(),
            "--lr=0.03".to_string(),
            "--tags=argv,tag".to_string(),
            "--backend.batch-size=128".to_string(),
            "--backend.arch=toy".to_string(),
        ])
        .with_env([
            ("KW_LR", "0.02"),
            ("KW_CACHE", "true"),
            ("KW_MODE", "debug"),
            ("KW_TAGS", "env,tag"),
            ("KW_SCHEDULE", "[env-schedule, env-extra]"),
            ("KW_RETRIES", "3"),
            ("KW_MAYBE_LABEL", "env-label"),
            ("KW_NUMBERS", "[7, 8]"),
            ("KW_METADATA", "{\"source\":\"env\"}"),
            ("KW_BACKEND_LABELS", "env-label,env-extra"),
            ("KW_BACKEND_AUGS", "[env-aug]"),
        ]);

    let cfg = FeatureMatrixConfig::from_sources(sources).unwrap();

    assert_eq!(cfg.lr, 0.03, "argv wins over env and config");
    assert!(cfg.cache, "env wins over config");
    assert_eq!(cfg.mode, "debug", "env wins over config when argv is absent");
    assert_eq!(cfg.tags, vec!["argv".to_string(), "tag".to_string()]);
    assert_eq!(cfg.schedule, vec!["env-schedule".to_string(), "env-extra".to_string()]);
    assert_eq!(cfg.retries, 3);
    assert_eq!(cfg.maybe_label, Some("env-label".to_string()));
    assert_eq!(cfg.numbers, vec![7, 8]);
    assert_eq!(cfg.metadata, json!({"source": "env"}));
    assert_eq!(cfg.backend.architecture, "toy", "nested alias in argv wins");
    assert_eq!(cfg.backend.batch_size, 128);
    assert_eq!(cfg.backend.labels, vec!["env-label".to_string(), "env-extra".to_string()]);
    assert_eq!(cfg.backend.augmentations, vec!["env-aug".to_string()]);

    let _ = std::fs::remove_file(path);
}

#[test]
fn explicit_config_value_is_used_instead_of_config_path() {
    let path = temp_config(
        "toml",
        r#"
lr = 9.0
mode = 'debug'
[backend]
batch_size = 999
"#,
    );

    let sources = kwconf::Sources::empty()
        .with_args(["feature-matrix"])
        .with_config_path(path.clone())
        .with_config_value(json!({
            "lr": 0.4,
            "mode": "safe",
            "backend": {
                "batch_size": 10,
                "architecture": "vit"
            }
        }));

    let cfg = FeatureMatrixConfig::from_sources(sources).unwrap();
    assert_eq!(cfg.lr, 0.4);
    assert_eq!(cfg.mode, "safe");
    assert_eq!(cfg.backend.batch_size, 10);
    assert_eq!(cfg.backend.architecture, "vit");

    let _ = std::fs::remove_file(path);
}

#[test]
fn config_file_loader_accepts_toml_json_yaml_yml_and_extension_fallbacks() {
    let cases = [
        (
            "toml",
            "lr = 0.11\nmode = 'safe'\n[backend]\nbatch_size = 41\narchitecture = 'vit'\n",
            0.11,
            "safe",
            41,
            "vit",
        ),
        (
            "json",
            r#"{"lr":0.12,"mode":"debug","backend":{"batch_size":42,"architecture":"toy"}}"#,
            0.12,
            "debug",
            42,
            "toy",
        ),
        (
            "yaml",
            "lr: 0.13\nmode: safe\nbackend:\n  batch_size: 43\n  architecture: resnet\n",
            0.13,
            "safe",
            43,
            "resnet",
        ),
        (
            "yml",
            "lr: 0.14\nmode: debug\nbackend:\n  batch_size: 44\n  architecture: vit\n",
            0.14,
            "debug",
            44,
            "vit",
        ),
        (
            "conf",
            r#"{"lr":0.15,"mode":"safe","backend":{"batch_size":45,"architecture":"toy"}}"#,
            0.15,
            "safe",
            45,
            "toy",
        ),
    ];

    for (ext, body, expected_lr, expected_mode, expected_batch, expected_arch) in cases {
        let path = temp_config(ext, body);
        let cfg = parse_feature([
            "feature-matrix".to_string(),
            "--config".to_string(),
            path.display().to_string(),
        ])
        .unwrap();

        assert_eq!(cfg.lr, expected_lr, "case {ext}");
        assert_eq!(cfg.mode, expected_mode, "case {ext}");
        assert_eq!(cfg.backend.batch_size, expected_batch, "case {ext}");
        assert_eq!(cfg.backend.architecture, expected_arch, "case {ext}");

        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn auto_parser_accepts_scalar_json_array_json_object_and_null_string_sources() {
    let cfg = parse_feature([
        "feature-matrix",
        "--cache=false",
        "--retries=5",
        "--maybe-label=null",
        "--numbers=[1,2,3]",
        "--metadata={\"name\":\"demo\",\"ok\":true}",
    ])
    .unwrap();

    assert!(!cfg.cache);
    assert_eq!(cfg.retries, 5);
    assert_eq!(cfg.maybe_label, None);
    assert_eq!(cfg.numbers, vec![1, 2, 3]);
    assert_eq!(cfg.metadata, json!({"name": "demo", "ok": true}));
}

#[test]
fn csv_and_yaml_parsers_are_used_for_argv_and_env_string_sources() {
    let sources = kwconf::Sources::empty()
        .with_args([
            "feature-matrix",
            "--tags=one, two,three",
            "--backend.labels=cat, dog",
        ])
        .with_env([
            ("KW_SCHEDULE", "[crop, flip]"),
            ("KW_BACKEND_AUGS", "[blur, sharpen]"),
        ]);

    let cfg = FeatureMatrixConfig::from_sources(sources).unwrap();
    assert_eq!(cfg.tags, vec!["one".to_string(), "two".to_string(), "three".to_string()]);
    assert_eq!(cfg.backend.labels, vec!["cat".to_string(), "dog".to_string()]);
    assert_eq!(cfg.schedule, vec!["crop".to_string(), "flip".to_string()]);
    assert_eq!(cfg.backend.augmentations, vec!["blur".to_string(), "sharpen".to_string()]);
}

#[test]
fn aliases_and_hyphen_underscore_normalization_apply_to_leaf_and_nested_fields() {
    let cfg = parse_feature([
        "feature-matrix",
        "--run-mode=safe",
        "--backend.batch-size=96",
        "--backend.bs=111",
        "--backend.arch=vit",
    ])
    .unwrap();

    assert_eq!(cfg.mode, "safe");
    assert_eq!(cfg.backend.batch_size, 111, "later alias assignment wins");
    assert_eq!(cfg.backend.architecture, "vit");
}

#[test]
fn choices_are_checked_for_config_env_and_argv_sources() {
    let config_err = FeatureMatrixConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["feature-matrix"])
            .with_config_value(json!({"mode": "turbo"})),
    )
    .unwrap_err();
    assert!(config_err.to_string().contains("invalid value for mode"));

    let env_err = FeatureMatrixConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["feature-matrix"])
            .with_env_pair("KW_MODE", "turbo"),
    )
    .unwrap_err();
    assert!(env_err.to_string().contains("invalid value for mode"));

    let argv_err = parse_feature(["feature-matrix", "--backend.architecture=cnn"]).unwrap_err();
    assert!(argv_err.to_string().contains("invalid value for architecture"));
}

#[test]
fn unknown_config_fields_are_rejected_at_top_level_and_inside_subconfigs() {
    let top_err = FeatureMatrixConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["feature-matrix"])
            .with_config_value(json!({"missing": 1})),
    )
    .unwrap_err();
    assert!(top_err.to_string().contains("unknown config value field: missing"));

    let nested_err = FeatureMatrixConfig::from_sources(
        kwconf::Sources::empty()
            .with_args(["feature-matrix"])
            .with_config_value(json!({"backend": {"missing": 1}})),
    )
    .unwrap_err();
    assert!(nested_err.to_string().contains("unknown config value field: missing"));
}

#[test]
fn reserved_runtime_options_report_missing_or_invalid_values() {
    let missing_config = parse_feature(["feature-matrix", "--config"]).unwrap_err();
    assert!(missing_config.to_string().contains("missing value for --config"));

    let missing_completion =
        parse_feature(["feature-matrix", "--generate-completion"]).unwrap_err();
    assert!(missing_completion
        .to_string()
        .contains("missing value for --generate-completion"));

    let bad_shell = parse_feature(["feature-matrix", "--generate-completion=nu"]).unwrap_err();
    assert!(bad_shell.to_string().contains("invalid completion shell"));

    let bad_color = parse_feature(["feature-matrix", "--color=rainbow"]).unwrap_err();
    assert!(bad_color.to_string().contains("invalid color choice"));
}

#[test]
fn help_and_completions_cover_runtime_flags_nested_fields_aliases_and_shells() {
    let help = FeatureMatrixConfig::help_with_color(kwconf::ColorChoice::Never);
    assert!(help.contains("Exercise supported kwconf-rs features."));
    assert!(help.contains("--config"));
    assert!(help.contains("--color"));
    assert!(help.contains("--generate-completion"));
    assert!(help.contains("--lr"));
    assert!(help.contains("--mode"));
    assert!(help.contains("--backend.architecture"));
    assert!(help.contains("--backend.batch-size"));
    assert!(help.contains("KW_TAGS"));
    assert!(help.contains("choices=fast|safe|debug"));

    let color_help = FeatureMatrixConfig::help_with_color(kwconf::ColorChoice::Always);
    assert!(color_help.contains("\x1b["));

    for shell in [
        kwconf::CompletionShell::Bash,
        kwconf::CompletionShell::Elvish,
        kwconf::CompletionShell::Fish,
        kwconf::CompletionShell::PowerShell,
        kwconf::CompletionShell::Zsh,
    ] {
        let script = FeatureMatrixConfig::completion_script(shell, "feature-matrix");
        assert_completion_mentions_long_option(&script, shell, "lr");
        assert_completion_mentions_long_option(&script, shell, "backend.architecture");
        assert_completion_mentions_long_option(&script, shell, "generate-completion");
    }
}

fn assert_completion_mentions_long_option(
    script: &str,
    shell: kwconf::CompletionShell,
    long_name: &str,
) {
    let double_dash = format!("--{long_name}");
    let fish_long = format!("-l {long_name}");
    let found = match shell {
        kwconf::CompletionShell::Fish => {
            script.contains(&double_dash) || script.contains(&fish_long)
        }
        _ => script.contains(&double_dash),
    };
    assert!(
        found,
        "missing --{long_name} in {shell:?} completion script:\n{script}"
    );
}
