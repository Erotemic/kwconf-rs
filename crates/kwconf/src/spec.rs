//! Static metadata produced by the derive macros.
//!
//! Everything here is `#[doc(hidden)]` plumbing between `kwconf_derive` and
//! the runtime. It is not part of the supported public API.

use std::collections::HashMap;

/// Static description of a config struct.
#[derive(Debug)]
pub struct ConfigSpec {
    pub name: &'static str,
    pub about: Option<&'static str>,
    pub fields: Vec<FieldInfo>,
    pub special_options: SpecialOptions,
}

/// Static description of a config field.
#[derive(Debug, Clone, Copy)]
pub struct FieldInfo {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub env: Option<&'static str>,
    pub help: Option<&'static str>,
    pub parser: Parser,
    pub choices: &'static [&'static str],
    pub kind: FieldKind,
    pub value_type: ValueType,
}

/// The shape of a config field.
#[derive(Debug, Clone, Copy)]
pub enum FieldKind {
    /// A normal leaf value.
    Value,
    /// A nested config object.
    Subconfig(&'static ConfigSpec),
}

/// Runtime-reserved CLI options enabled for a config or modal command.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpecialOptions {
    pub config: bool,
    pub color: bool,
    pub completion: bool,
}

/// Coarse field type metadata used for kwconf-style bool negation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Other,
    Bool,
}

/// Static description of a modal enum.
#[derive(Debug)]
pub struct ModalSpec {
    pub name: &'static str,
    pub about: Option<&'static str>,
    pub variants: Vec<ModalVariantInfo>,
    pub default_variant: Option<&'static str>,
    pub special_options: SpecialOptions,
}

/// Static description of a modal enum variant.
#[derive(Debug, Clone, Copy)]
pub struct ModalVariantInfo {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub help: Option<&'static str>,
    pub spec: &'static ConfigSpec,
}

/// Parser used for string-only sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parser {
    Auto,
    Csv,
    Yaml,
}

impl Parser {
    pub fn name(self) -> &'static str {
        match self {
            Parser::Auto => "auto",
            Parser::Csv => "csv",
            Parser::Yaml => "yaml",
        }
    }
}

/// A path from a root spec to one leaf field.
pub(crate) type FieldPath = Vec<&'static FieldInfo>;

/// The leaf field whose `choices` apply to a path.
pub(crate) fn choice_field<'a>(path: &[&'a FieldInfo]) -> &'a FieldInfo {
    path.last().expect("field path is non-empty")
}

pub(crate) fn dotted_name(path: &[&FieldInfo]) -> String {
    path.iter()
        .map(|field| field.name)
        .collect::<Vec<_>>()
        .join(".")
}

/// Dash/underscore-insensitive key comparison form.
pub(crate) fn normalize_key(key: &str) -> String {
    key.trim_start_matches('-').replace('-', "_")
}

pub(crate) fn normalize_path(key: &str) -> String {
    key.trim_start_matches('-')
        .split('.')
        .map(normalize_key)
        .collect::<Vec<_>>()
        .join(".")
}

/// Dash/underscore-insensitive equality without allocating.
pub(crate) fn key_eq(a: &str, b: &str) -> bool {
    let fold = |ch: char| if ch == '-' { '_' } else { ch };
    let a = a.trim_start_matches('-');
    let b = b.trim_start_matches('-');
    a.len() == b.len() && a.chars().map(fold).eq(b.chars().map(fold))
}

/// Find a direct field by canonical name or alias.
pub(crate) fn find_field<'a>(spec: &'a ConfigSpec, key: &str) -> Option<&'a FieldInfo> {
    spec.fields.iter().find(|field| {
        key_eq(field.name, key) || field.aliases.iter().any(|alias| key_eq(alias, key))
    })
}

/// Find a leaf or subconfig by dotted path.
pub(crate) fn find_field_path(spec: &'static ConfigSpec, key: &str) -> Option<FieldPath> {
    let mut spec = spec;
    let mut path = Vec::new();
    let parts: Vec<&str> = key.split('.').collect();
    for (index, part) in parts.iter().enumerate() {
        let field = find_field(spec, part)?;
        path.push(field);
        if index + 1 < parts.len() {
            match field.kind {
                FieldKind::Subconfig(child) => spec = child,
                FieldKind::Value => return None,
            }
        }
    }
    Some(path)
}

/// Every leaf path in declaration order.
pub(crate) fn leaf_paths(spec: &'static ConfigSpec) -> Vec<FieldPath> {
    fn walk(spec: &'static ConfigSpec, prefix: &mut FieldPath, out: &mut Vec<FieldPath>) {
        for field in &spec.fields {
            prefix.push(field);
            match field.kind {
                FieldKind::Value => out.push(prefix.clone()),
                FieldKind::Subconfig(child) => walk(child, prefix, out),
            }
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    walk(spec, &mut Vec::new(), &mut out);
    out
}

/// A set of CLI option names with dash/underscore-insensitive collision detection.
#[derive(Default)]
pub(crate) struct Namespace {
    claimed: HashMap<String, String>,
}

impl Namespace {
    pub(crate) fn claim(&mut self, long: &str, owner: impl Into<String>) -> Result<(), String> {
        let owner = owner.into();
        let key = normalize_key(long);
        if let Some(existing) = self.claimed.get(&key) {
            if *existing == owner {
                return Ok(());
            }
            return Err(format!(
                "option --{long} is claimed by both {existing} and {owner}"
            ));
        }
        self.claimed.insert(key, owner);
        Ok(())
    }
}
