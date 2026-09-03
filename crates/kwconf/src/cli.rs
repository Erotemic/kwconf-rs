//! Lightweight typed argv resolution.
//!
//! This module deliberately has no Serde dependency. `clap` recognizes the
//! command grammar; derive-generated setters perform the final `FromStr`
//! conversion into Rust fields.

use crate::command::{
    build_config_model, build_modal_model, extract, map_clap_error, normalize_argv,
    completion_request, render_help, render_subcommand_help,
};
use crate::spec::{
    dotted_name, key_eq, FieldPath, ModalSpec, ModalVariantInfo,
};
use crate::{Cli, Error, Result};
use clap::ColorChoice;
use std::ffi::OsString;
use std::fmt::Display;
use std::str::FromStr;

pub(crate) fn resolve_cli<T, I, A>(args: I) -> Result<T>
where
    T: Cli,
    I: IntoIterator<Item = A>,
    A: Into<OsString>,
{
    let spec = T::cli_spec();
    if spec.special_options.config {
        return Err(Error::Schema(
            "the lightweight Cli API cannot enable special_options(config); use Config".to_string(),
        ));
    }
    let mut model = build_config_model(spec, spec.name)?;
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let args = normalize_argv(&args, spec.name);
    let matches = model
        .command
        .clone()
        .try_get_matches_from(args)
        .map_err(|err| map_clap_error(err, &model.bindings))?;
    let parsed = extract(&matches, spec.special_options, &model.bindings)?;
    let color = parsed.color.unwrap_or(ColorChoice::Auto);

    if parsed.help {
        return Err(Error::HelpRequested(render_help(&mut model.command, color)));
    }
    if let Some(shell) = parsed.completion {
        return Err(completion_request(model.command, shell, spec.name));
    }

    let mut value = T::default();
    apply_cli_values(&mut value, parsed.values)?;
    Ok(value)
}

fn apply_cli_values<T: Cli>(value: &mut T, values: Vec<(FieldPath, String)>) -> Result<()> {
    for (path, text) in values {
        let full_name = dotted_name(&path);
        value.__kwconf_apply_cli(&path, &full_name, &text)?;
    }
    Ok(())
}

/// Parse one scalar CLI value through ordinary Rust `FromStr`.
#[doc(hidden)]
pub fn parse_cli_value<T>(field: &str, text: &str) -> Result<T>
where
    T: FromStr,
    T::Err: Display,
{
    text.parse::<T>().map_err(|err| Error::InvalidValue {
        field: field.to_string(),
        message: err.to_string(),
    })
}

/// Parse the strict kwconf boolean spelling used by both Cli and Config.
#[doc(hidden)]
pub fn parse_cli_bool(field: &str, text: &str) -> Result<bool> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("true") || text == "1" {
        return Ok(true);
    }
    if text.eq_ignore_ascii_case("false") || text == "0" {
        return Ok(false);
    }
    Err(Error::InvalidValue {
        field: field.to_string(),
        message: format!("expected true/false or 1/0, got {text:?}"),
    })
}

/// Parse an optional CLI scalar. `none` and `null` mean `None`.
#[doc(hidden)]
pub fn parse_cli_optional<T>(field: &str, text: &str) -> Result<Option<T>>
where
    T: FromStr,
    T::Err: Display,
{
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("none") || trimmed.eq_ignore_ascii_case("null") {
        Ok(None)
    } else {
        parse_cli_value(field, text).map(Some)
    }
}

/// Parse a comma-separated CLI value while preserving explicit empty fields.
///
/// `a,,b,` becomes four elements: `a`, empty, `b`, empty. This intentionally
/// differs from Python kwconf's current filtering of empty CSV components.
#[doc(hidden)]
pub fn parse_cli_csv<T>(field: &str, text: &str) -> Result<Vec<T>>
where
    T: FromStr,
    T::Err: Display,
{
    text.split(',')
        .map(str::trim)
        .map(|part| parse_cli_value(field, part))
        .collect()
}

/// Parse an optional CSV value. `none` and `null` mean `None`; an empty
/// string is an explicit one-element CSV containing the empty field.
#[doc(hidden)]
pub fn parse_cli_optional_csv<T>(field: &str, text: &str) -> Result<Option<Vec<T>>>
where
    T: FromStr,
    T::Err: Display,
{
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("none") || trimmed.eq_ignore_ascii_case("null") {
        Ok(None)
    } else {
        parse_cli_csv(field, text).map(Some)
    }
}

/// Selection produced by the lightweight modal parser.
#[doc(hidden)]
pub struct ModalCliSelection {
    variant: &'static ModalVariantInfo,
    argv: Vec<(FieldPath, String)>,
}

impl ModalCliSelection {
    pub fn variant(&self) -> &'static str {
        self.variant.name
    }
}

/// Parse modal argv and choose a variant without config/env machinery.
#[doc(hidden)]
pub fn resolve_modal_cli_selection<I, A>(
    spec: &'static ModalSpec,
    args: I,
) -> Result<ModalCliSelection>
where
    I: IntoIterator<Item = A>,
    A: Into<OsString>,
{
    if spec.special_options.config {
        return Err(Error::Schema(
            "the lightweight ModalCli API cannot enable special_options(config); use ModalConfig"
                .to_string(),
        ));
    }
    let mut model = build_modal_model(spec)?;
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let args = normalize_argv(&args, spec.name);
    let matches = model
        .command
        .clone()
        .try_get_matches_from(args)
        .map_err(|err| {
            map_clap_error(
                err,
                model.variants.iter().flat_map(|variant| &variant.bindings),
            )
        })?;
    let root = extract(&matches, spec.special_options, &[])?;

    if root.help {
        let color = root.color.unwrap_or(ColorChoice::Auto);
        return Err(Error::HelpRequested(render_help(&mut model.command, color)));
    }
    if let Some(shell) = root.completion {
        return Err(completion_request(model.command, shell, spec.name));
    }

    let subcommand = matches.subcommand();
    let variant_name = if let Some((name, _)) = subcommand {
        name.to_string()
    } else if let Some(name) = spec.default_variant {
        name.to_string()
    } else {
        spec.variants
            .first()
            .map(|variant| variant.name.to_string())
            .ok_or_else(|| Error::Message("modal CLI has no variants".to_string()))?
    };

    let index = spec
        .variants
        .iter()
        .position(|variant| {
            key_eq(variant.name, &variant_name)
                || variant.aliases.iter().any(|alias| key_eq(alias, &variant_name))
        })
        .ok_or_else(|| Error::InvalidModalVariant(variant_name.clone()))?;
    let variant = &model.variants[index];

    let child = match subcommand {
        Some((_, sub_matches)) => extract(
            sub_matches,
            variant.info.spec.special_options,
            &variant.bindings,
        )?,
        None => Default::default(),
    };
    let color = child.color.or(root.color).unwrap_or(ColorChoice::Auto);
    if child.help {
        return Err(Error::HelpRequested(render_subcommand_help(
            &mut model.command,
            variant.info.name,
            color,
        )));
    }
    if let Some(shell) = child.completion {
        let child_model = build_config_model(variant.info.spec, variant.info.name)?;
        return Err(completion_request(
            child_model.command,
            shell,
            variant.info.name,
        ));
    }

    Ok(ModalCliSelection {
        variant: variant.info,
        argv: child.values,
    })
}

/// Build one selected modal payload from its argv assignments.
#[doc(hidden)]
pub fn resolve_modal_cli_variant<T: Cli>(selection: ModalCliSelection) -> Result<T> {
    let mut value = T::default();
    apply_cli_values(&mut value, selection.argv)?;
    Ok(value)
}
