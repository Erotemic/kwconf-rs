use clap::ColorChoice;
use std::fmt;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

/// Result type used by kwconf-rs.
pub type Result<T> = std::result::Result<T, Error>;

/// Rendered help text together with the color policy that was requested for it.
#[derive(Debug, Clone)]
pub struct Help {
    plain: String,
    ansi: String,
    color: ColorChoice,
}

impl Help {
    pub(crate) fn new(styled: &clap::builder::StyledStr, color: ColorChoice) -> Self {
        Self {
            plain: styled.to_string(),
            ansi: styled.ansi().to_string(),
            color,
        }
    }

    /// Help text without ANSI styling.
    pub fn plain(&self) -> &str {
        &self.plain
    }

    /// Help text with ANSI styling.
    pub fn ansi(&self) -> &str {
        &self.ansi
    }

    /// The color policy that was requested when the help was rendered.
    pub fn color(&self) -> ColorChoice {
        self.color
    }

    /// The text selected by the color policy; `Auto` resolves to plain text.
    pub fn text(&self) -> &str {
        match self.color {
            ColorChoice::Always => &self.ansi,
            ColorChoice::Never | ColorChoice::Auto => &self.plain,
        }
    }

    /// Print the help to stdout, resolving `Auto` from the terminal and `NO_COLOR`.
    pub fn print(&self) -> std::io::Result<()> {
        let use_ansi = match self.color {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                std::io::stdout().is_terminal()
                    && std::env::var_os("NO_COLOR").is_none()
                    && std::env::var_os("TERM").as_deref() != Some("dumb".as_ref())
            }
        };
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(if use_ansi { &self.ansi } else { &self.plain }.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()
    }
}

impl fmt::Display for Help {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.text())
    }
}

/// Errors returned by kwconf-rs.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// `--help` was requested; `cli()` prints this and exits successfully.
    HelpRequested(Help),
    /// `--generate-completion` was requested; the payload is the script.
    CompletionRequested(String),
    /// A config file could not be read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A config file could not be parsed.
    ConfigFormat {
        path: PathBuf,
        message: String,
    },
    /// The derived schema is internally inconsistent (for example two fields
    /// claim the same CLI option).
    Schema(String),
    /// argv contained an option the schema does not declare.
    UnknownArgument(String),
    /// A config file or value contained a key the schema does not declare.
    UnknownField {
        field: String,
        source: &'static str,
    },
    /// An option that requires a value was given without one.
    MissingValue(String),
    InvalidCompletionShell(String),
    InvalidColorChoice(String),
    InvalidModalVariant(String),
    /// A value did not match the field's declared `choices`.
    Choice {
        field: String,
        value: String,
        choices: &'static [&'static str],
    },
    /// The merged sources could not be deserialized into the config type.
    /// `field` is the dotted path when it is known, otherwise empty.
    Deserialize {
        field: String,
        message: String,
    },
    /// Any other error, already formatted for the user.
    Message(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::HelpRequested(help) => f.write_str(help.text()),
            Error::CompletionRequested(script) => f.write_str(script),
            Error::Io { path, source } => write!(f, "failed to read {}: {source}", path.display()),
            Error::ConfigFormat { path, message } => {
                write!(f, "failed to parse {}: {message}", path.display())
            }
            Error::Schema(message) => write!(f, "invalid kwconf schema: {message}"),
            Error::UnknownArgument(arg) => write!(f, "unknown argument: {arg}"),
            Error::UnknownField { field, source } => write!(f, "unknown {source} field: {field}"),
            Error::MissingValue(arg) => write!(f, "missing value for {arg}"),
            Error::InvalidCompletionShell(shell) => write!(
                f,
                "invalid completion shell: {shell:?}. Expected one of: bash, elvish, fish, powershell, zsh"
            ),
            Error::InvalidColorChoice(choice) => write!(
                f,
                "invalid color choice: {choice:?}. Expected one of: auto, always, never"
            ),
            Error::InvalidModalVariant(name) => write!(f, "invalid modal variant: {name}"),
            Error::Choice {
                field,
                value,
                choices,
            } => write!(
                f,
                "invalid value for {field}: {value:?}. Expected one of: {}",
                choices.join(", ")
            ),
            Error::Deserialize { field, message } if field.is_empty() => {
                write!(f, "failed to deserialize config: {message}")
            }
            Error::Deserialize { field, message } => {
                write!(f, "invalid value for {field}: {message}")
            }
            Error::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
