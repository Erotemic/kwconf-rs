//! The single `clap` command model.
//!
//! One `Command` is built from the derived schema and used for argv
//! recognition, help rendering, and completion scripts, so there is exactly
//! one interpretation of the CLI.

use crate::error::{Error, Help, Result};
use crate::spec::{
    dotted_name, normalize_key, ConfigSpec, FieldInfo, FieldKind, FieldPath, ModalSpec,
    ModalVariantInfo, Namespace, Parser, SpecialOptions, ValueType,
};
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::builder::PossibleValuesParser;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::parser::ValueSource;
use clap::{Arg, ArgAction, ArgMatches, ColorChoice, Command};
use clap_complete::aot::{generate, Shell};
use std::ffi::OsString;
use std::path::PathBuf;

pub(crate) const HELP_ID: &str = "help";
pub(crate) const CONFIG_ID: &str = "config";
pub(crate) const COLOR_ID: &str = "color";
pub(crate) const COMPLETION_ID: &str = "generate-completion";
const SHELL_NAMES: [&str; 5] = ["bash", "elvish", "fish", "powershell", "zsh"];
const COLOR_NAMES: [&str; 3] = ["auto", "always", "never"];

/// Links one clap argument id to the field it sets.
pub(crate) struct Binding {
    pub id: String,
    pub path: FieldPath,
    /// `--no-flag` style negation of a bool field.
    pub negated: bool,
}

/// A config command plus the bindings needed to read its matches.
pub(crate) struct ConfigModel {
    pub command: Command,
    pub bindings: Vec<Binding>,
}

/// A modal command plus one model per variant.
pub(crate) struct ModalModel {
    pub command: Command,
    pub variants: Vec<VariantModel>,
}

pub(crate) struct VariantModel {
    pub info: &'static ModalVariantInfo,
    pub bindings: Vec<Binding>,
}

/// Everything recognized in one command's argv.
#[derive(Debug, Default)]
pub(crate) struct ParsedArgs {
    pub help: bool,
    pub color: Option<ColorChoice>,
    pub completion: Option<Shell>,
    pub config_path: Option<PathBuf>,
    /// Field assignments in argv order.
    pub values: Vec<(FieldPath, String)>,
}

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
        .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
        .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
        .placeholder(AnsiColor::Cyan.on_default())
        .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
        .valid(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
        .invalid(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
}

fn base_command(name: &str, about: Option<&'static str>) -> Command {
    let mut cmd = Command::new(name.to_string())
        .styles(styles())
        .disable_help_flag(true)
        .disable_help_subcommand(true)
        .disable_version_flag(true)
        .arg_required_else_help(false);
    if let Some(about) = about {
        cmd = cmd.about(about);
    }
    cmd
}

fn help_arg() -> Arg {
    Arg::new(HELP_ID)
        .short('h')
        .long("help")
        .action(ArgAction::SetTrue)
        .help("Print help")
}

fn schema(message: String) -> Error {
    Error::Schema(message)
}

fn add_special_options(
    mut cmd: Command,
    options: SpecialOptions,
    namespace: &mut Namespace,
) -> Result<Command> {
    namespace
        .claim("help", "the built-in --help flag")
        .map_err(schema)?;
    if options.config {
        namespace
            .claim("config", "the --config special option")
            .map_err(schema)?;
        cmd = cmd.arg(
            Arg::new(CONFIG_ID)
                .long("config")
                .value_name("PATH")
                .action(ArgAction::Set)
                .value_parser(clap::value_parser!(PathBuf))
                .help("Read TOML, JSON, YAML, or YML config."),
        );
    }
    if options.completion {
        namespace
            .claim(
                "generate-completion",
                "the --generate-completion special option",
            )
            .map_err(schema)?;
        cmd = cmd.arg(
            Arg::new(COMPLETION_ID)
                .long("generate-completion")
                .value_name("SHELL")
                .action(ArgAction::Set)
                .value_parser(PossibleValuesParser::new(SHELL_NAMES))
                .help("Generate a shell completion script."),
        );
    }
    if options.color {
        namespace
            .claim("color", "the --color special option")
            .map_err(schema)?;
        cmd = cmd.arg(
            Arg::new(COLOR_ID)
                .long("color")
                .value_name("WHEN")
                .action(ArgAction::Set)
                .value_parser(PossibleValuesParser::new(COLOR_NAMES))
                .help("Control help color: auto, always, or never."),
        );
    }
    Ok(cmd)
}

/// Build the command for one config struct.
pub(crate) fn build_config_model(spec: &'static ConfigSpec, name: &str) -> Result<ConfigModel> {
    let mut namespace = Namespace::default();
    let mut cmd = base_command(name, spec.about);
    cmd = add_special_options(cmd, spec.special_options, &mut namespace)?;
    let mut bindings = Vec::new();
    cmd = add_fields(cmd, spec, &mut Vec::new(), &mut bindings, &mut namespace)?;
    cmd = cmd.arg(help_arg());
    Ok(ConfigModel {
        command: cmd,
        bindings,
    })
}

fn dashed(name: &str) -> String {
    name.replace('_', "-")
}

/// All spellings of a path where each component may use its name or an alias.
fn path_spellings(path: &[&FieldInfo]) -> Vec<Vec<&'static str>> {
    let mut out: Vec<Vec<&'static str>> = vec![Vec::new()];
    for field in path {
        let mut next = Vec::new();
        for prefix in &out {
            for name in std::iter::once(&field.name).chain(field.aliases.iter()) {
                let mut spelled = prefix.clone();
                spelled.push(name);
                next.push(spelled);
            }
        }
        out = next;
    }
    out
}

fn add_fields(
    mut cmd: Command,
    spec: &'static ConfigSpec,
    prefix: &mut FieldPath,
    bindings: &mut Vec<Binding>,
    namespace: &mut Namespace,
) -> Result<Command> {
    for field in &spec.fields {
        prefix.push(field);
        match field.kind {
            FieldKind::Subconfig(child) => {
                cmd = add_fields(cmd, child, prefix, bindings, namespace)?;
            }
            FieldKind::Value => {
                cmd = add_value_arg(cmd, prefix, bindings, namespace)?;
            }
        }
        prefix.pop();
    }
    Ok(cmd)
}

fn add_value_arg(
    mut cmd: Command,
    path: &FieldPath,
    bindings: &mut Vec<Binding>,
    namespace: &mut Namespace,
) -> Result<Command> {
    let field = *path.last().expect("field path is non-empty");
    let id = dotted_name(path);
    let long = dashed(&id);
    let owner = format!("field {id}");
    let is_bool = matches!(field.value_type, ValueType::Bool);

    namespace.claim(&long, owner.clone()).map_err(schema)?;

    let mut visible = Vec::new();
    let mut hidden = Vec::new();
    if long != id {
        visible.push(id.clone());
    }
    let parent_len = path.len() - 1;
    for spelling in path_spellings(path).into_iter().skip(1) {
        let spelled = dashed(&spelling.join("."));
        namespace.claim(&spelled, owner.clone()).map_err(schema)?;
        let parents_canonical = spelling[..parent_len]
            .iter()
            .zip(path.iter())
            .all(|(name, field)| *name == field.name);
        if parents_canonical {
            visible.push(spelled);
        } else {
            hidden.push(spelled);
        }
    }

    let mut arg = Arg::new(id.clone())
        .long(long.clone())
        .value_name("VALUE")
        .action(ArgAction::Append)
        .num_args(1)
        .allow_negative_numbers(true)
        .help(field_help(field));
    if !visible.is_empty() {
        arg = arg.visible_aliases(visible);
    }
    if !hidden.is_empty() {
        arg = arg.aliases(hidden);
    }
    if is_bool {
        arg = arg.num_args(0..=1).default_missing_value("true");
    }
    if !field.choices.is_empty() {
        arg = arg.value_parser(PossibleValuesParser::new(field.choices.iter().copied()));
    }
    cmd = cmd.arg(arg);
    bindings.push(Binding {
        id: id.clone(),
        path: path.clone(),
        negated: false,
    });

    if is_bool {
        let neg_id = format!("no-{id}");
        let neg_long = format!("no-{long}");
        let neg_owner = format!("the negation of bool field {id}");
        namespace
            .claim(&neg_long, neg_owner.clone())
            .map_err(schema)?;
        let mut neg_visible = Vec::new();
        let mut neg_hidden = Vec::new();
        if neg_long != neg_id {
            neg_visible.push(neg_id.clone());
        }
        for spelling in path_spellings(path).into_iter().skip(1) {
            let spelled = format!("no-{}", dashed(&spelling.join(".")));
            namespace
                .claim(&spelled, neg_owner.clone())
                .map_err(schema)?;
            neg_hidden.push(spelled);
        }
        let mut neg = Arg::new(neg_id.clone())
            .long(neg_long)
            .action(ArgAction::Count)
            .help(format!("Set --{long} to false."));
        if !neg_visible.is_empty() {
            neg = neg.visible_aliases(neg_visible);
        }
        if !neg_hidden.is_empty() {
            neg = neg.aliases(neg_hidden);
        }
        cmd = cmd.arg(neg);
        bindings.push(Binding {
            id: neg_id,
            path: path.clone(),
            negated: true,
        });
    }
    Ok(cmd)
}

fn field_help(field: &FieldInfo) -> String {
    let mut parts = Vec::new();
    if let Some(help) = field.help {
        parts.push(help.to_string());
    }
    if field.parser != Parser::Auto {
        parts.push(format!("parser={}", field.parser.name()));
    }
    if let Some(env) = field.env {
        parts.push(format!("env={env}"));
    }
    if !field.choices.is_empty() {
        parts.push(format!("choices={}", field.choices.join("|")));
    }
    parts.join(" ")
}

/// Build the command for one modal enum.
pub(crate) fn build_modal_model(spec: &'static ModalSpec) -> Result<ModalModel> {
    let mut namespace = Namespace::default();
    let mut cmd = base_command(spec.name, spec.about).subcommand_required(false);
    cmd = add_special_options(cmd, spec.special_options, &mut namespace)?;
    cmd = cmd.arg(help_arg());

    let mut names = Namespace::default();
    let mut variants = Vec::new();
    for variant in &spec.variants {
        let owner = format!("modal variant {}", variant.name);
        for name in std::iter::once(&variant.name).chain(variant.aliases.iter()) {
            names
                .claim(name, owner.clone())
                .map_err(|message| schema(message.replacen("option --", "subcommand ", 1)))?;
        }
        let child = build_config_model(variant.spec, variant.name)?;
        let mut sub = child.command;
        if let Some(help) = variant.help {
            sub = sub.about(help);
        }
        for alias in variant.aliases {
            sub = sub.visible_alias(dashed(alias));
        }
        cmd = cmd.subcommand(sub);
        variants.push(VariantModel {
            info: variant,
            bindings: child.bindings,
        });
    }

    Ok(ModalModel {
        command: cmd,
        variants,
    })
}

/// Normalize argv for clap: option names use dashes, `--` ends normalization.
pub(crate) fn normalize_argv(args: &[OsString], fallback_name: &str) -> Vec<OsString> {
    let mut out = Vec::with_capacity(args.len().max(1));
    let mut iter = args.iter();
    match iter.next() {
        Some(program) => out.push(program.clone()),
        None => out.push(OsString::from(fallback_name)),
    }
    let mut literal = false;
    for arg in iter {
        if literal {
            out.push(arg.clone());
            continue;
        }
        let Some(text) = arg.to_str() else {
            out.push(arg.clone());
            continue;
        };
        if text == "--" {
            literal = true;
            out.push(arg.clone());
            continue;
        }
        if let Some(body) = text.strip_prefix("--") {
            let (key, value) = match body.split_once('=') {
                Some((key, value)) => (key, Some(value)),
                None => (body, None),
            };
            let mut rebuilt = String::with_capacity(text.len());
            rebuilt.push_str("--");
            rebuilt.push_str(&dashed(key));
            if let Some(value) = value {
                rebuilt.push('=');
                rebuilt.push_str(value);
            }
            out.push(OsString::from(rebuilt));
        } else {
            out.push(arg.clone());
        }
    }
    out
}

pub(crate) fn parse_shell(text: &str) -> Result<Shell> {
    match text.to_ascii_lowercase().as_str() {
        "bash" => Ok(Shell::Bash),
        "elvish" => Ok(Shell::Elvish),
        "fish" => Ok(Shell::Fish),
        "powershell" | "power-shell" | "pwsh" => Ok(Shell::PowerShell),
        "zsh" => Ok(Shell::Zsh),
        _ => Err(Error::InvalidCompletionShell(text.to_string())),
    }
}

pub(crate) fn parse_color_choice(text: &str) -> Result<ColorChoice> {
    match text.to_ascii_lowercase().as_str() {
        "auto" => Ok(ColorChoice::Auto),
        "always" => Ok(ColorChoice::Always),
        "never" => Ok(ColorChoice::Never),
        _ => Err(Error::InvalidColorChoice(text.to_string())),
    }
}

/// Read one command's matches into ordered field assignments.
pub(crate) fn extract(
    matches: &ArgMatches,
    special: SpecialOptions,
    bindings: &[Binding],
) -> Result<ParsedArgs> {
    let from_cli = |id: &str| {
        matches.try_get_raw(id).is_ok()
            && matches.value_source(id) == Some(ValueSource::CommandLine)
    };

    let mut parsed = ParsedArgs {
        help: from_cli(HELP_ID) && matches.get_flag(HELP_ID),
        ..ParsedArgs::default()
    };
    if special.color && from_cli(COLOR_ID) {
        if let Some(text) = matches.get_one::<String>(COLOR_ID) {
            parsed.color = Some(parse_color_choice(text)?);
        }
    }
    if special.completion && from_cli(COMPLETION_ID) {
        if let Some(text) = matches.get_one::<String>(COMPLETION_ID) {
            parsed.completion = Some(parse_shell(text)?);
        }
    }
    if special.config && from_cli(CONFIG_ID) {
        parsed.config_path = matches.get_one::<PathBuf>(CONFIG_ID).cloned();
    }

    let mut ordered: Vec<(usize, &Binding, String)> = Vec::new();
    for binding in bindings {
        if !from_cli(&binding.id) {
            continue;
        }
        let indices = matches.indices_of(&binding.id).into_iter().flatten();
        if binding.negated {
            for index in indices {
                ordered.push((index, binding, "false".to_string()));
            }
        } else if let Some(values) = matches.get_many::<String>(&binding.id) {
            for (index, value) in indices.zip(values) {
                ordered.push((index, binding, value.clone()));
            }
        }
    }
    ordered.sort_by_key(|(index, _, _)| *index);
    parsed.values = ordered
        .into_iter()
        .map(|(_, binding, value)| (binding.path.clone(), value))
        .collect();
    Ok(parsed)
}

/// Translate a clap parse error into a kwconf error.
pub(crate) fn map_clap_error<'a>(
    err: clap::Error,
    bindings: impl IntoIterator<Item = &'a Binding>,
) -> Error {
    let context_string = |kind: ContextKind| match err.get(kind) {
        Some(ContextValue::String(text)) => Some(text.clone()),
        _ => None,
    };
    let context_strings = |kind: ContextKind| match err.get(kind) {
        Some(ContextValue::Strings(items)) => items.clone(),
        Some(ContextValue::String(text)) => vec![text.clone()],
        _ => Vec::new(),
    };

    match err.kind() {
        ErrorKind::UnknownArgument => {
            let arg = context_string(ContextKind::InvalidArg).unwrap_or_default();
            let suggestions = context_strings(ContextKind::SuggestedArg);
            if suggestions.is_empty() {
                Error::UnknownArgument(arg)
            } else {
                Error::UnknownArgument(format!(
                    "{arg} (did you mean {}?)",
                    suggestions.join(" or ")
                ))
            }
        }
        ErrorKind::InvalidSubcommand => Error::InvalidModalVariant(
            context_string(ContextKind::InvalidSubcommand).unwrap_or_default(),
        ),
        ErrorKind::InvalidValue => {
            let arg = context_string(ContextKind::InvalidArg).unwrap_or_default();
            let long = arg
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            let value = context_string(ContextKind::InvalidValue).unwrap_or_default();
            if value.is_empty() {
                return Error::MissingValue(long);
            }
            match long.as_str() {
                "--color" => Error::InvalidColorChoice(value),
                "--generate-completion" => Error::InvalidCompletionShell(value),
                _ => {
                    let wanted = normalize_key(&long);
                    let field = bindings
                        .into_iter()
                        .find(|binding| !binding.negated && normalize_key(&binding.id) == wanted)
                        .and_then(|binding| binding.path.last().copied());
                    match field {
                        Some(field) if !field.choices.is_empty() => Error::Choice {
                            field: normalize_key(&long),
                            value,
                            choices: field.choices,
                        },
                        _ => Error::Message(render_clap_message(&err)),
                    }
                }
            }
        }
        _ => Error::Message(render_clap_message(&err)),
    }
}

fn render_clap_message(err: &clap::Error) -> String {
    let rendered = err.render().to_string();
    let first_line = rendered.lines().next().unwrap_or_default();
    first_line
        .strip_prefix("error: ")
        .unwrap_or(first_line)
        .to_string()
}

pub(crate) fn render_help(cmd: &mut Command, color: ColorChoice) -> Help {
    Help::new(&cmd.render_help(), color)
}

pub(crate) fn render_completion(mut cmd: Command, shell: Shell, bin_name: &str) -> String {
    cmd.set_bin_name(bin_name.to_string());
    let mut buf = Vec::new();
    generate(shell, &mut cmd, bin_name.to_string(), &mut buf);
    String::from_utf8(buf).expect("clap completion output is UTF-8")
}

/// Render help for one subcommand of a built modal command.
pub(crate) fn render_subcommand_help(
    root: &mut Command,
    variant: &str,
    color: ColorChoice,
) -> Help {
    root.build();
    match root.find_subcommand_mut(variant) {
        Some(sub) => render_help(sub, color),
        None => render_help(root, color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_normalization_only_touches_option_names() {
        let args: Vec<OsString> = ["prog", "--a_b.c_d=x_y", "--flag", "sub_cmd", "--", "--k_v"]
            .into_iter()
            .map(OsString::from)
            .collect();
        let out = normalize_argv(&args, "prog");
        let out: Vec<&str> = out.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(
            out,
            ["prog", "--a-b.c-d=x_y", "--flag", "sub_cmd", "--", "--k_v"]
        );
        assert_eq!(normalize_argv(&[], "prog").len(), 1);
    }

    #[test]
    fn shell_and_color_parsers_accept_common_names() {
        assert_eq!(parse_shell("bash").unwrap(), Shell::Bash);
        assert_eq!(parse_shell("pwsh").unwrap(), Shell::PowerShell);
        assert!(matches!(
            parse_color_choice("auto").unwrap(),
            ColorChoice::Auto
        ));
        assert!(matches!(
            parse_color_choice("always").unwrap(),
            ColorChoice::Always
        ));
        assert!(matches!(
            parse_color_choice("never").unwrap(),
            ColorChoice::Never
        ));
    }
}
