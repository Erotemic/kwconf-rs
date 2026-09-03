#[derive(Debug, Clone, PartialEq, kwconf::Cli)]
struct OptimizerCli {
    /// Learning rate.
    #[kwconf(default = 0.001)]
    lr: f64,
}

#[derive(Debug, Clone, PartialEq, kwconf::Cli)]
#[kwconf(name = "train", special_options(color, generate_completion))]
struct TrainCli {
    /// Port used by the local service.
    #[kwconf(default = 8080, alias = "listen-port")]
    port: u16,

    /// Enable verbose logging.
    verbose: bool,

    /// Comma-separated tags. Explicit empty fields are preserved.
    #[kwconf(parser = "csv")]
    tags: Vec<String>,

    /// Optional retry count.
    retries: Option<u8>,

    #[kwconf(subconfig)]
    optimizer: OptimizerCli,
}

#[test]
fn cli_derive_needs_no_serde_on_the_user_type() {
    let cfg = TrainCli::from_iter([
        "train",
        "--port=9000",
        "--verbose",
        "--tags=a,,b,",
        "--retries=3",
        "--optimizer.lr=0.02",
    ])
    .unwrap();

    assert_eq!(cfg.port, 9000);
    assert!(cfg.verbose);
    assert_eq!(cfg.tags, vec!["a", "", "b", ""]);
    assert_eq!(cfg.retries, Some(3));
    assert_eq!(cfg.optimizer.lr, 0.02);
}

#[test]
fn cli_bool_spellings_are_strict_and_last_assignment_wins() {
    assert!(TrainCli::from_iter(["train", "--verbose=1"]).unwrap().verbose);
    assert!(!TrainCli::from_iter(["train", "--verbose=0"]).unwrap().verbose);
    assert!(!TrainCli::from_iter(["train", "--verbose", "--no-verbose"])
        .unwrap()
        .verbose);
    assert!(TrainCli::from_iter(["train", "--verbose=yes"]).is_err());
}

#[test]
fn doc_comments_feed_help_without_duplicate_help_metadata() {
    let help = TrainCli::help();
    assert!(help.contains("Port used by the local service."), "{help}");
    assert!(help.contains("Enable verbose logging."), "{help}");
    assert!(help.contains("--optimizer.lr"), "{help}");
}

#[derive(Debug, Clone, PartialEq, kwconf::Cli)]
struct EvalCli {
    #[kwconf(default = "model.pt")]
    checkpoint: String,
}

#[derive(Debug, Clone, PartialEq, kwconf::ModalCli)]
#[kwconf(name = "tool")]
enum ToolCli {
    #[kwconf(default)]
    Train(TrainCli),
    #[kwconf(alias = "test")]
    Eval(EvalCli),
}

#[test]
fn modal_cli_uses_clap_subcommands_without_config_or_serde() {
    match ToolCli::from_iter(["tool", "eval", "--checkpoint=best.pt"]).unwrap() {
        ToolCli::Eval(cfg) => assert_eq!(cfg.checkpoint, "best.pt"),
        other => panic!("expected eval, got {other:?}"),
    }

    match ToolCli::from_iter(["tool", "test", "--checkpoint=alias.pt"]).unwrap() {
        ToolCli::Eval(cfg) => assert_eq!(cfg.checkpoint, "alias.pt"),
        other => panic!("expected eval alias, got {other:?}"),
    }
}
