//! Layered source resolution: defaults < config file < env < argv.
//!
//! Resolution starts from `T::default()` and mutates typed fields directly.
//! The config struct itself is never round-tripped through Serde.

use crate::command::{
    build_config_model, build_modal_model, extract, map_clap_error, normalize_argv,
    completion_request, render_help, render_subcommand_help,
};
use crate::error::{Error, Result};
use crate::spec::{
    choice_field, dotted_name, find_field, find_field_path, key_eq, leaf_paths, ConfigSpec,
    FieldKind, FieldPath, ModalSpec, ModalVariantInfo,
};
use crate::tree::choice_text;
use crate::{Config, Sources};
use clap::ColorChoice;
use serde_json::{Map, Value};
use std::path::Path;

/// One structured config layer and where it came from.
type Layer = (Value, &'static str);

pub(crate) fn resolve_config<T: Config>(mut sources: Sources) -> Result<T> {
    let spec = T::config_spec();
    let mut model = build_config_model(spec, spec.name)?;
    let args = normalize_argv(sources.args(), spec.name);
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

    let mut layers = Vec::new();
    if let Some(value) = sources.take_config_value() {
        layers.push((value, "config value"));
    } else if let Some(path) = parsed.config_path.or_else(|| sources.take_config_path()) {
        layers.push((read_config_file(&path)?, "config file"));
    }

    resolve_layers::<T>(spec, layers, &sources, parsed.values)
}

fn resolve_layers<T: Config>(
    spec: &'static ConfigSpec,
    layers: Vec<Layer>,
    sources: &Sources,
    argv: Vec<(FieldPath, String)>,
) -> Result<T> {
    let mut config = T::default();

    for (value, source) in layers {
        apply_config_object(&mut config, spec, value, source, &mut Vec::new())?;
    }

    for path in leaf_paths(spec) {
        let field = *path.last().expect("field path is non-empty");
        let Some(env_name) = field.env else {
            continue;
        };
        if let Some(text) = sources.env_value(env_name)? {
            check_choice_text(&path, &text)?;
            let full_name = dotted_name(&path);
            config.__kwconf_apply_raw(&path, &full_name, &text, "env")?;
        }
    }

    for (path, text) in argv {
        check_choice_text(&path, &text)?;
        let full_name = dotted_name(&path);
        config.__kwconf_apply_raw(&path, &full_name, &text, "argv")?;
    }

    Ok(config)
}

fn apply_config_object<T: Config>(
    config: &mut T,
    spec: &'static ConfigSpec,
    value: Value,
    source: &'static str,
    prefix: &mut FieldPath,
) -> Result<()> {
    let Value::Object(map) = value else {
        return Err(Error::Message(format!(
            "{source} must contain an object at the top level"
        )));
    };
    apply_map(config, spec, map, source, prefix)
}

fn apply_map<T: Config>(
    config: &mut T,
    spec: &'static ConfigSpec,
    map: Map<String, Value>,
    source: &'static str,
    prefix: &mut FieldPath,
) -> Result<()> {
    for (key, value) in map {
        let normalized = crate::spec::normalize_path(&key);

        if normalized.contains('.') {
            let local = find_field_path(spec, &normalized).ok_or_else(|| Error::UnknownField {
                field: normalized.clone(),
                source,
            })?;
            let mut full = prefix.clone();
            full.extend(local.iter().copied());
            apply_path_value(config, &full, value, source)?;
            continue;
        }

        let field = find_field(spec, &normalized).ok_or_else(|| Error::UnknownField {
            field: normalized.clone(),
            source,
        })?;
        prefix.push(field);
        match field.kind {
            FieldKind::Value => {
                check_choice_value(prefix, &value)?;
                let full_name = dotted_name(prefix);
                config.__kwconf_apply_value(prefix, &full_name, value, source)?;
            }
            FieldKind::Subconfig(child) => {
                let Value::Object(child_map) = value else {
                    return Err(Error::Message(format!(
                        "{source} field {} must contain an object",
                        dotted_name(prefix)
                    )));
                };
                apply_map(config, child, child_map, source, prefix)?;
            }
        }
        prefix.pop();
    }
    Ok(())
}

fn apply_path_value<T: Config>(
    config: &mut T,
    path: &FieldPath,
    value: Value,
    source: &'static str,
) -> Result<()> {
    let Some(field) = path.last() else {
        return Err(Error::Schema("empty field path".to_string()));
    };
    match field.kind {
        FieldKind::Value => {
            check_choice_value(path, &value)?;
            let full_name = dotted_name(path);
            config.__kwconf_apply_value(path, &full_name, value, source)
        }
        FieldKind::Subconfig(child) => {
            let Value::Object(map) = value else {
                return Err(Error::Message(format!(
                    "{source} field {} must contain an object",
                    dotted_name(path)
                )));
            };
            let mut prefix = path.clone();
            apply_map(config, child, map, source, &mut prefix)
        }
    }
}

fn check_choice_text(path: &[&crate::spec::FieldInfo], text: &str) -> Result<()> {
    let field = choice_field(path);
    if field.choices.is_empty() || field.choices.contains(&text) {
        return Ok(());
    }
    Err(Error::Choice {
        field: dotted_name(path),
        value: text.to_string(),
        choices: field.choices,
    })
}

fn check_choice_value(path: &[&crate::spec::FieldInfo], value: &Value) -> Result<()> {
    let field = choice_field(path);
    if field.choices.is_empty() {
        return Ok(());
    }
    match choice_text(value) {
        Some(text) => check_choice_text(path, &text),
        None => Ok(()),
    }
}

pub(crate) fn read_config_file(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let format_error = |message: String| Error::ConfigFormat {
        path: path.to_path_buf(),
        message,
    };

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "json" => parse_json(&text).map_err(format_error),
        "toml" => parse_toml(&text).map_err(format_error),
        "yaml" | "yml" => parse_yaml(&text).map_err(format_error),
        _ => parse_unknown_extension(&text).map_err(format_error),
    }
}

fn parse_json(text: &str) -> std::result::Result<Value, String> {
    serde_json::from_str(text).map_err(|err| err.to_string())
}

fn parse_toml(text: &str) -> std::result::Result<Value, String> {
    #[cfg(feature = "toml")]
    {
        let value: toml::Value = toml::from_str(text).map_err(|err| err.to_string())?;
        return serde_json::to_value(value).map_err(|err| err.to_string());
    }

    #[cfg(not(feature = "toml"))]
    {
        let _ = text;
        Err("TOML support requires Cargo feature `toml`".to_string())
    }
}

fn parse_yaml(text: &str) -> std::result::Result<Value, String> {
    #[cfg(feature = "yaml")]
    {
        return yaml_serde::from_str(text).map_err(|err| err.to_string());
    }

    #[cfg(not(feature = "yaml"))]
    {
        let _ = text;
        Err("YAML support requires Cargo feature `yaml`".to_string())
    }
}

fn parse_unknown_extension(text: &str) -> std::result::Result<Value, String> {
    let mut errors = Vec::new();

    #[cfg(feature = "toml")]
    match parse_toml(text) {
        Ok(value) => return Ok(value),
        Err(err) => errors.push(format!("TOML ({err})")),
    }

    match parse_json(text) {
        Ok(value) => return Ok(value),
        Err(err) => errors.push(format!("JSON ({err})")),
    }

    #[cfg(feature = "yaml")]
    match parse_yaml(text) {
        Ok(value) => return Ok(value),
        Err(err) => errors.push(format!("YAML ({err})")),
    }

    Err(format!(
        "not valid in any enabled config format: {}",
        errors.join("; ")
    ))
}

/// Resolved modal selection handed to derive-generated code.
pub struct ModalSelection {
    variant: &'static ModalVariantInfo,
    layers: Vec<Layer>,
    sources: Sources,
    argv: Vec<(FieldPath, String)>,
}

impl ModalSelection {
    /// Canonical name of the selected variant.
    pub fn variant(&self) -> &'static str {
        self.variant.name
    }
}

/// Parse modal argv and pick one variant.
pub fn resolve_modal_selection(
    spec: &'static ModalSpec,
    mut sources: Sources,
) -> Result<ModalSelection> {
    let mut model = build_modal_model(spec)?;
    let args = normalize_argv(sources.args(), spec.name);
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

    let (config_value, config_source) = if let Some(value) = sources.take_config_value() {
        (Some(value), "config value")
    } else if let Some(path) = root.config_path.or_else(|| sources.take_config_path()) {
        (Some(read_config_file(&path)?), "config file")
    } else {
        (None, "config file")
    };

    let subcommand = matches.subcommand();
    let variant_name = if let Some((name, _)) = subcommand {
        name.to_string()
    } else if let Some(name) = modal_variant_from_config(config_value.as_ref()) {
        name
    } else if let Some(name) = spec.default_variant {
        name.to_string()
    } else {
        spec.variants
            .first()
            .map(|variant| variant.name.to_string())
            .ok_or_else(|| Error::Message("modal config has no variants".to_string()))?
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

    let mut layers = Vec::new();
    if let Some(value) = config_value {
        if let Some(table) = modal_child_config_value(spec, variant.info, value) {
            layers.push((table, config_source));
        }
    }
    if let Some(path) = child.config_path {
        layers.push((read_config_file(&path)?, "config file"));
    }

    Ok(ModalSelection {
        variant: variant.info,
        layers,
        sources,
        argv: child.values,
    })
}

/// Resolve the selected variant's payload config.
pub fn resolve_modal_variant<T: Config>(selection: ModalSelection) -> Result<T> {
    resolve_layers::<T>(
        selection.variant.spec,
        selection.layers,
        &selection.sources,
        selection.argv,
    )
}

fn modal_variant_from_config(config_value: Option<&Value>) -> Option<String> {
    let Some(Value::Object(map)) = config_value else {
        return None;
    };
    map.get("command")
        .or_else(|| map.get("mode"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn modal_child_config_value(
    spec: &'static ModalSpec,
    selected: &'static ModalVariantInfo,
    value: Value,
) -> Option<Value> {
    let Value::Object(map) = value else {
        return None;
    };

    let matches_variant = |key: &str, variant: &ModalVariantInfo| {
        key_eq(variant.name, key) || variant.aliases.iter().any(|alias| key_eq(alias, key))
    };

    let mut remaining = Map::new();
    let mut selected_table = None;
    for (key, value) in map {
        if selected_table.is_none() && matches_variant(&key, selected) {
            selected_table = Some(value);
            continue;
        }
        let is_selector = key == "command" || key == "mode";
        let is_other_variant = spec
            .variants
            .iter()
            .any(|variant| matches_variant(&key, variant));
        if is_selector || is_other_variant {
            continue;
        }
        remaining.insert(key, value);
    }

    if let Some(table) = selected_table {
        return Some(table);
    }
    if remaining.is_empty() {
        None
    } else {
        Some(Value::Object(remaining))
    }
}

#[cfg(test)]
mod format_feature_tests {
    use super::*;

    #[test]
    fn json_is_part_of_the_base_config_feature() {
        let value = parse_json(r#"{"answer": 42}"#).unwrap();
        assert_eq!(value["answer"], 42);
    }

    #[cfg(feature = "toml")]
    #[test]
    fn toml_parser_is_available_when_enabled() {
        let value = parse_toml("answer = 42").unwrap();
        assert_eq!(value["answer"], 42);
    }

    #[cfg(not(feature = "toml"))]
    #[test]
    fn toml_parser_reports_the_missing_feature() {
        let err = parse_toml("answer = 42").unwrap_err();
        assert!(err.contains("Cargo feature `toml`"));
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn yaml_parser_is_available_when_enabled() {
        let value = parse_yaml("answer: 42").unwrap();
        assert_eq!(value["answer"], 42);
    }

    #[cfg(not(feature = "yaml"))]
    #[test]
    fn yaml_parser_reports_the_missing_feature() {
        let err = parse_yaml("answer: 42").unwrap_err();
        assert!(err.contains("Cargo feature `yaml`"));
    }
}
