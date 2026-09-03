//! Source merging: defaults < config file < env < argv.

use crate::command::{
    build_config_model, build_modal_model, extract, map_clap_error, normalize_argv,
    render_completion, render_help, render_subcommand_help, ParsedArgs,
};
use crate::error::{Error, Result};
use crate::spec::{
    choice_field, dotted_name, find_field, find_field_path, key_eq, leaf_paths, normalize_path,
    ConfigSpec, FieldKind, FieldPath, ModalSpec, ModalVariantInfo,
};
use crate::tree::{choice_text, Node, RawToken};
use crate::{Config, Sources};
use clap::ColorChoice;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
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
        return Err(Error::CompletionRequested(render_completion(
            model.command,
            shell,
            spec.name,
        )));
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
    let mut root = defaults::<T>()?;
    for (value, source) in layers {
        merge_object(spec, &mut root, value, source)?;
    }
    apply_env(spec, &mut root, sources)?;
    for (path, text) in argv {
        let field = *path.last().expect("field path is non-empty");
        check_choice_text(&path, &text)?;
        let token = RawToken {
            text,
            parser: field.parser,
            source: "argv",
        };
        set_path(&mut root, &path, Node::Raw(token))?;
    }
    T::deserialize(Node::Object(root)).map_err(|err| Error::Deserialize {
        field: err.path.join("."),
        message: err.message,
    })
}

fn defaults<T: Config>() -> Result<BTreeMap<String, Node>> {
    let value = serde_json::to_value(T::default()).map_err(|err| Error::Deserialize {
        field: String::new(),
        message: format!("defaults could not be serialized: {err}"),
    })?;
    match value {
        Value::Object(map) => Ok(Node::from_object(map)),
        _ => Err(Error::Message(
            "config defaults must serialize as an object".to_string(),
        )),
    }
}

fn merge_object(
    spec: &'static ConfigSpec,
    root: &mut BTreeMap<String, Node>,
    value: Value,
    source: &'static str,
) -> Result<()> {
    let Value::Object(map) = value else {
        return Err(Error::Message(format!(
            "{source} must contain an object at the top level"
        )));
    };

    for (key, value) in map {
        let key = normalize_path(&key);
        if key.contains('.') {
            let path = find_field_path(spec, &key).ok_or_else(|| Error::UnknownField {
                field: key.clone(),
                source,
            })?;
            match path.last().map(|field| field.kind) {
                Some(FieldKind::Subconfig(child)) => {
                    let target = ensure_object(root, &path)?;
                    merge_object(child, target, value, source)?;
                }
                _ => {
                    check_choice_value(&path, &value)?;
                    set_path(root, &path, Node::Value(value))?;
                }
            }
            continue;
        }

        let field = find_field(spec, &key).ok_or_else(|| Error::UnknownField {
            field: key.clone(),
            source,
        })?;
        match field.kind {
            FieldKind::Value => {
                check_choice_value(&[field], &value)?;
                root.insert(field.name.to_string(), Node::Value(value));
            }
            FieldKind::Subconfig(child) => {
                if !value.is_object() {
                    return Err(Error::Message(format!(
                        "{source} field {} must contain an object",
                        field.name
                    )));
                }
                let target = ensure_object(root, &[field])?;
                merge_object(child, target, value, source)?;
            }
        }
    }
    Ok(())
}

fn apply_env(
    spec: &'static ConfigSpec,
    root: &mut BTreeMap<String, Node>,
    sources: &Sources,
) -> Result<()> {
    for path in leaf_paths(spec) {
        let field = *path.last().expect("field path is non-empty");
        let Some(env_name) = field.env else {
            continue;
        };
        if let Some(text) = sources.env_value(env_name)? {
            check_choice_text(&path, &text)?;
            let token = RawToken {
                text,
                parser: field.parser,
                source: "env",
            };
            set_path(root, &path, Node::Raw(token))?;
        }
    }
    Ok(())
}

/// Walk to the object that holds the last element of `path`, creating objects as needed.
fn ensure_object<'a>(
    root: &'a mut BTreeMap<String, Node>,
    path: &[&crate::spec::FieldInfo],
) -> Result<&'a mut BTreeMap<String, Node>> {
    let mut current = root;
    for field in path {
        let entry = current
            .entry(field.name.to_string())
            .or_insert_with(|| Node::Object(BTreeMap::new()));
        if let Node::Value(Value::Object(_)) = entry {
            let Node::Value(Value::Object(map)) =
                std::mem::replace(entry, Node::Object(BTreeMap::new()))
            else {
                unreachable!()
            };
            *entry = Node::Object(Node::from_object(map));
        }
        match entry {
            Node::Object(map) => current = map,
            _ => {
                return Err(Error::Message(format!(
                    "field {} must contain an object",
                    field.name
                )))
            }
        }
    }
    Ok(current)
}

fn set_path(root: &mut BTreeMap<String, Node>, path: &FieldPath, node: Node) -> Result<()> {
    let (leaf, parents) = path.split_last().expect("field path is non-empty");
    let target = ensure_object(root, parents)?;
    target.insert(leaf.name.to_string(), node);
    Ok(())
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
        "toml" => parse_toml(&text).map_err(format_error),
        "json" => serde_json::from_str(&text).map_err(|err| format_error(err.to_string())),
        "yaml" | "yml" => yaml_serde::from_str(&text).map_err(|err| format_error(err.to_string())),
        _ => {
            let toml_err = match parse_toml(&text) {
                Ok(value) => return Ok(value),
                Err(err) => err,
            };
            let json_err = match serde_json::from_str::<Value>(&text) {
                Ok(value) => return Ok(value),
                Err(err) => err.to_string(),
            };
            let yaml_err = match yaml_serde::from_str::<Value>(&text) {
                Ok(value) => return Ok(value),
                Err(err) => err.to_string(),
            };
            Err(format_error(format!(
                "not valid TOML ({toml_err}), JSON ({json_err}), or YAML ({yaml_err})"
            )))
        }
    }
}

fn parse_toml(text: &str) -> std::result::Result<Value, String> {
    let value: toml::Value = toml::from_str(text).map_err(|err| err.to_string())?;
    serde_json::to_value(value).map_err(|err| err.to_string())
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
        return Err(Error::CompletionRequested(render_completion(
            model.command,
            shell,
            spec.name,
        )));
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
                || variant
                    .aliases
                    .iter()
                    .any(|alias| key_eq(alias, &variant_name))
        })
        .ok_or_else(|| Error::InvalidModalVariant(variant_name.clone()))?;
    let variant = &model.variants[index];

    let child = match subcommand {
        Some((_, sub_matches)) => extract(
            sub_matches,
            variant.info.config_spec.special_options,
            &variant.bindings,
        )?,
        None => ParsedArgs::default(),
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
        let child_model = build_config_model(variant.info.config_spec, variant.info.name)?;
        return Err(Error::CompletionRequested(render_completion(
            child_model.command,
            shell,
            variant.info.name,
        )));
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
        selection.variant.config_spec,
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

/// Pick the selected variant's table out of a modal config value.
///
/// A file may either keep one table per variant or be flat for the selected
/// variant. Table keys are matched dash/underscore-insensitively.
fn modal_child_config_value(
    spec: &ModalSpec,
    selected: &ModalVariantInfo,
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
