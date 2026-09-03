#[derive(Debug, kwconf::Cli)]
/// Small typed CLI with no Serde or config-file dependency.
struct Args {
    /// Port to listen on.
    #[kwconf(default = 8080)]
    port: u16,

    /// Enable verbose logging.
    verbose: bool,

    /// Comma-separated labels.
    #[kwconf(parser = "csv")]
    labels: Vec<String>,
}

fn main() {
    let args = Args::cli();
    println!("{args:#?}");
}
