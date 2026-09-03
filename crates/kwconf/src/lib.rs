//! Python-style typed CLI and layered configuration for Rust.
//!
//! `kwconf` has two layers:
//!
//! - [`Cli`] is the lightweight argv-only interface. It needs neither Serde nor
//!   config-file support. With the `derive` feature, `#[derive(kwconf::Cli)]`
//!   provides the normal Python-like struct API.
//! - [`Config`] (behind the default `config` feature) adds environment and
//!   structured config sources with the precedence contract
//!   `defaults < config file < env < argv`.
//!
//! One `clap` command model owns argv recognition, help, aliases, subcommands,
//! and completions for both layers. The derive macros generate metadata and
//! typed field setters; they do not implement a second command-line parser.

#![forbid(unsafe_code)]

mod cli;
mod command;
mod error;
#[cfg(feature = "config")]
mod resolve;
#[cfg(feature = "config")]
mod sources;
mod spec;
#[cfg(feature = "config")]
mod tree;

use std::ffi::OsString;

pub use clap::ColorChoice;
pub use clap_complete::aot::Shell as CompletionShell;
pub use error::{Error, Help, Result};

#[cfg(feature = "derive")]
pub use kwconf_derive::{Cli, ModalCli};
#[cfg(all(feature = "derive", feature = "config"))]
pub use kwconf_derive::{Config, ModalConfig};

#[cfg(feature = "config")]
pub use sources::Sources;

/// Derive-macro plumbing. Not part of the supported public API.
#[doc(hidden)]
pub mod __private {
    pub use crate::cli::{
        parse_cli_bool, parse_cli_csv, parse_cli_optional, parse_cli_optional_csv, parse_cli_value,
        resolve_modal_cli_selection, resolve_modal_cli_variant, ModalCliSelection,
    };
    pub use crate::spec::{
        ConfigSpec, FieldInfo, FieldKind, ModalSpec, ModalVariantInfo, Parser, SpecialOptions,
        ValueType,
    };

    #[cfg(feature = "config")]
    pub use crate::resolve::{resolve_modal_selection, resolve_modal_variant, ModalSelection};
    #[cfg(feature = "config")]
    pub use crate::tree::{parse_config_raw, parse_config_value};
    #[cfg(feature = "config")]
    pub use serde;
    #[cfg(feature = "config")]
    pub use serde_json;
}

/// A typed argv-only command.
///
/// This trait has no Serde dependency. Most callers use
/// `#[derive(kwconf::Cli)]`; the trait remains public so the runtime does not
/// fundamentally depend on procedural macros.
pub trait Cli: Default + Sized {
    /// Static command metadata.
    #[doc(hidden)]
    fn cli_spec() -> &'static __private::ConfigSpec;

    /// Apply one canonical field path parsed from argv.
    #[doc(hidden)]
    fn __kwconf_apply_cli(
        &mut self,
        path: &[&'static __private::FieldInfo],
        full_name: &str,
        text: &str,
    ) -> Result<()>;

    /// Parse explicit argv, including the program name.
    #[allow(clippy::should_implement_trait)]
    fn from_iter<I, T>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        cli::resolve_cli::<Self, _, _>(args)
    }

    /// Parse the current process argv.
    fn try_cli() -> Result<Self> {
        Self::from_iter(std::env::args_os())
    }

    /// Parse the current process argv or exit with a user-facing message.
    fn cli() -> Self {
        Self::try_cli().unwrap_or_else(exit_with)
    }

    /// Render help text for this command.
    ///
    /// # Panics
    ///
    /// Panics if the derived schema is invalid.
    fn help() -> String {
        Self::help_with_color(ColorChoice::Auto)
    }

    /// Render help with an explicit color policy.
    ///
    /// # Panics
    ///
    /// Panics if the derived schema is invalid.
    fn help_with_color(color: ColorChoice) -> String {
        let spec = Self::cli_spec();
        let mut model = command::build_config_model(spec, spec.name).unwrap_or_else(schema_panic);
        command::render_help(&mut model.command, color).to_string()
    }

    /// Generate a shell completion script.
    ///
    /// # Panics
    ///
    /// Panics if the derived schema is invalid.
    fn completion_script(shell: CompletionShell, bin_name: &str) -> String {
        let spec = Self::cli_spec();
        let model = command::build_config_model(spec, spec.name).unwrap_or_else(schema_panic);
        command::render_completion(model.command, shell, bin_name)
    }
}

/// A typed argv-only modal/subcommand enum.
pub trait ModalCli: Sized {
    /// Static modal metadata.
    #[doc(hidden)]
    fn modal_cli_spec() -> &'static __private::ModalSpec;

    /// Parse explicit argv, including the program name.
    fn from_iter<I, T>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>;

    /// Parse the current process argv.
    fn try_cli() -> Result<Self> {
        Self::from_iter(std::env::args_os())
    }

    /// Parse the current process argv or exit with a user-facing message.
    fn cli() -> Self {
        Self::try_cli().unwrap_or_else(exit_with)
    }

    /// Render modal help.
    fn help() -> String {
        Self::help_with_color(ColorChoice::Auto)
    }

    /// Render modal help with an explicit color policy.
    fn help_with_color(color: ColorChoice) -> String {
        let mut model =
            command::build_modal_model(Self::modal_cli_spec()).unwrap_or_else(schema_panic);
        command::render_help(&mut model.command, color).to_string()
    }

    /// Generate a shell completion script.
    fn completion_script(shell: CompletionShell, bin_name: &str) -> String {
        let model = command::build_modal_model(Self::modal_cli_spec()).unwrap_or_else(schema_panic);
        command::render_completion(model.command, shell, bin_name)
    }
}

/// A layered config object.
///
/// Unlike the previous implementation, the config struct itself does not need
/// to implement Serde traits. Resolution starts from `Self::default()` and
/// applies typed fields directly. Serde is used only where structured values
/// need to be decoded into leaf field types.
#[cfg(feature = "config")]
pub trait Config: Default + Sized {
    /// Static config metadata.
    #[doc(hidden)]
    fn config_spec() -> &'static __private::ConfigSpec;

    /// Apply one raw argv/env value to a canonical field path.
    #[doc(hidden)]
    fn __kwconf_apply_raw(
        &mut self,
        path: &[&'static __private::FieldInfo],
        full_name: &str,
        text: &str,
        source: &'static str,
    ) -> Result<()>;

    /// Apply one structured config-file value to a canonical field path.
    #[doc(hidden)]
    fn __kwconf_apply_value(
        &mut self,
        path: &[&'static __private::FieldInfo],
        full_name: &str,
        value: serde_json::Value,
        source: &'static str,
    ) -> Result<()>;

    /// Resolve this config from explicit sources.
    fn from_sources(sources: Sources) -> Result<Self> {
        resolve::resolve_config::<Self>(sources)
    }

    /// Resolve this config from explicit argv only.
    ///
    /// Unlike [`Config::try_cli`], this does not read the process environment.
    #[allow(clippy::should_implement_trait)]
    fn from_iter<I, T>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        Self::from_sources(Sources::empty().with_args(args))
    }

    /// Resolve from the current process argv and declared environment bindings.
    fn try_cli() -> Result<Self> {
        Self::from_sources(Sources::new())
    }

    /// Resolve from the current process or exit with a user-facing message.
    fn cli() -> Self {
        Self::try_cli().unwrap_or_else(exit_with)
    }

    /// Render help text for this config.
    fn help() -> String {
        Self::help_with_color(ColorChoice::Auto)
    }

    /// Render help with an explicit color policy.
    fn help_with_color(color: ColorChoice) -> String {
        let spec = Self::config_spec();
        let mut model = command::build_config_model(spec, spec.name).unwrap_or_else(schema_panic);
        command::render_help(&mut model.command, color).to_string()
    }

    /// Generate a shell completion script.
    fn completion_script(shell: CompletionShell, bin_name: &str) -> String {
        let spec = Self::config_spec();
        let model = command::build_config_model(spec, spec.name).unwrap_or_else(schema_panic);
        command::render_completion(model.command, shell, bin_name)
    }
}

/// A layered modal/subcommand config enum.
#[cfg(feature = "config")]
pub trait ModalConfig: Sized {
    /// Static modal metadata.
    #[doc(hidden)]
    fn modal_spec() -> &'static __private::ModalSpec;

    /// Resolve this modal config from explicit sources.
    fn from_sources(sources: Sources) -> Result<Self>;

    /// Resolve this modal config from explicit argv only.
    #[allow(clippy::should_implement_trait)]
    fn from_iter<I, T>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        Self::from_sources(Sources::empty().with_args(args))
    }

    /// Resolve from the current process argv and declared environment bindings.
    fn try_cli() -> Result<Self> {
        Self::from_sources(Sources::new())
    }

    /// Resolve from the current process or exit with a user-facing message.
    fn cli() -> Self {
        Self::try_cli().unwrap_or_else(exit_with)
    }

    /// Render modal help.
    fn help() -> String {
        Self::help_with_color(ColorChoice::Auto)
    }

    /// Render modal help with an explicit color policy.
    fn help_with_color(color: ColorChoice) -> String {
        let mut model = command::build_modal_model(Self::modal_spec()).unwrap_or_else(schema_panic);
        command::render_help(&mut model.command, color).to_string()
    }

    /// Generate a shell completion script.
    fn completion_script(shell: CompletionShell, bin_name: &str) -> String {
        let model = command::build_modal_model(Self::modal_spec()).unwrap_or_else(schema_panic);
        command::render_completion(model.command, shell, bin_name)
    }
}

fn schema_panic<T>(err: Error) -> T {
    panic!("{err}")
}

/// Print a resolution error the way `cli()` does and exit.
fn exit_with<T>(err: Error) -> T {
    match err {
        Error::HelpRequested(help) => {
            if let Err(io_err) = help.print() {
                eprintln!("failed to print help: {io_err}");
            }
            std::process::exit(0);
        }
        Error::CompletionRequested(script) => {
            println!("{script}");
            std::process::exit(0);
        }
        err => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    }
}
