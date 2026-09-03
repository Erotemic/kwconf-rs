use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(name = "reserved-free")]
struct ReservedFreeConfig {
    #[kwconf(default = "plain", help = "A normal user field named color.")]
    color: String,

    #[kwconf(
        default = "manual",
        help = "A normal user field named generate_completion."
    )]
    generate_completion: String,

    #[kwconf(default = "inline", help = "A normal user field named config.")]
    config: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, kwconf::Config)]
#[kwconf(
    name = "reserved-enabled",
    special_options(config, color, generate_completion)
)]
struct ReservedEnabledConfig {
    #[kwconf(default = "value", help = "Ordinary value.")]
    value: String,
}

#[test]
fn runtime_special_options_are_off_by_default() {
    let cfg = ReservedFreeConfig::from_iter([
        "reserved-free",
        "--color=blue",
        "--generate-completion=manual-page",
        "--config=project.toml",
    ])
    .unwrap();

    assert_eq!(cfg.color, "blue");
    assert_eq!(cfg.generate_completion, "manual-page");
    assert_eq!(cfg.config, "project.toml");

    let help = ReservedFreeConfig::help_with_color(kwconf::ColorChoice::Never);
    assert!(help.contains("--color <VALUE>"));
    assert!(help.contains("--generate-completion <VALUE>"));
    assert!(help.contains("--config <VALUE>"));
    assert!(!help.contains("possible values: bash"));
    assert!(!help.contains("Control help color"));
    assert!(!help.contains("Read TOML, JSON, YAML"));
}

#[test]
fn runtime_special_options_are_explicitly_opted_in() {
    let help = ReservedEnabledConfig::help_with_color(kwconf::ColorChoice::Never);
    assert!(help.contains("--config <PATH>"));
    assert!(help.contains("--generate-completion <SHELL>"));
    assert!(help.contains("--color <WHEN>"));

    let color_help =
        ReservedEnabledConfig::from_iter(["reserved-enabled", "--color=always", "--help"])
            .unwrap_err()
            .to_string();
    assert!(color_help.contains("\x1b["));

    let completion =
        ReservedEnabledConfig::from_iter(["reserved-enabled", "--generate-completion=bash"])
            .unwrap_err()
            .to_string();
    assert!(completion.contains("reserved-enabled"));
}
