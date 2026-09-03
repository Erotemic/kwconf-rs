//! Derive macros for [`kwconf`](https://crates.io/crates/kwconf).
//!
//! Use `kwconf::Config` and `kwconf::ModalConfig`; this crate is an
//! implementation detail re-exported by `kwconf`.

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use std::collections::HashMap;
use syn::{
    parse_macro_input, Data, DeriveInput, Expr, ExprArray, ExprLit, Fields, GenericParam, Generics,
    Lit, Type,
};

#[proc_macro_derive(Config, attributes(kwconf))]
pub fn derive_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_config(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro_derive(ModalConfig, attributes(kwconf))]
pub fn derive_modal_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_modal_config(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Dash/underscore-insensitive CLI name form.
fn normalize(name: &str) -> String {
    name.trim_start_matches('-').replace('-', "_")
}

/// Tracks the option namespace of one struct so collisions become compile errors.
#[derive(Default)]
struct NameSpace {
    claimed: HashMap<String, String>,
}

impl NameSpace {
    fn claim(&mut self, name: &str, owner: &str, span: &dyn ToTokens) -> syn::Result<()> {
        let key = normalize(name);
        if key.is_empty() {
            return Err(syn::Error::new_spanned(
                span,
                "option names cannot be empty",
            ));
        }
        if let Some(existing) = self.claimed.get(&key) {
            return Err(syn::Error::new_spanned(
                span,
                format!(
                    "kwconf option `{}` is claimed by both {existing} and {owner}",
                    key.replace('_', "-")
                ),
            ));
        }
        self.claimed.insert(key, owner.to_string());
        Ok(())
    }
}

fn expand_config(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ident = input.ident;
    let struct_opts = StructOpts::from_attrs(&input.attrs)?;
    let krate = struct_opts
        .krate
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(::kwconf));

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "kwconf::Config only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                ident,
                "kwconf::Config only supports structs",
            ));
        }
    };

    let mut names = NameSpace::default();
    names.claim("help", "the built-in --help flag", &ident)?;
    if struct_opts.special_options.config {
        names.claim("config", "the --config special option", &ident)?;
    }
    if struct_opts.special_options.color {
        names.claim("color", "the --color special option", &ident)?;
    }
    if struct_opts.special_options.completion {
        names.claim(
            "generate-completion",
            "the --generate-completion special option",
            &ident,
        )?;
    }

    let generic_idents: Vec<String> = input
        .generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(ty) => Some(ty.ident.to_string()),
            _ => None,
        })
        .collect();

    let mut default_fields = Vec::new();
    let mut infos = Vec::new();

    for field in fields {
        let field_ident = field.ident.expect("named fields have names");
        let field_name = field_ident.to_string().trim_start_matches("r#").to_string();
        let field_ty = field.ty;
        let opts = FieldOpts::from_attrs(&field.attrs)?;

        if opts.modal {
            return Err(syn::Error::new_spanned(
                field_ident,
                "#[kwconf(modal)] is reserved for a future inline modal field API; derive kwconf::ModalConfig on an enum instead",
            ));
        }
        if opts.subconfig && mentions_generic(&field_ty, &generic_idents) {
            return Err(syn::Error::new_spanned(
                field_ty,
                "#[kwconf(subconfig)] fields cannot use the struct's generic parameters",
            ));
        }

        let owner = format!("field `{field_name}`");
        names.claim(&field_name, &owner, &field_ident)?;
        for alias in &opts.aliases {
            names.claim(alias, &format!("alias `{alias}` of {owner}"), &field_ident)?;
        }
        if is_bool_type(&field_ty) && !opts.subconfig {
            names.claim(
                &format!("no-{field_name}"),
                &format!("the negation of bool {owner}"),
                &field_ident,
            )?;
            for alias in &opts.aliases {
                names.claim(
                    &format!("no-{alias}"),
                    &format!("the negation of alias `{alias}` of {owner}"),
                    &field_ident,
                )?;
            }
        }

        let default_expr = match opts.default {
            Some(Some(expr)) => default_expr_for_field(&field_ty, &expr),
            Some(None) | None => quote! { <#field_ty as ::core::default::Default>::default() },
        };
        default_fields.push(quote! { #field_ident: #default_expr });

        let parser = match opts.parser.as_deref().unwrap_or("auto") {
            "auto" => quote! { #krate::__private::Parser::Auto },
            "csv" => quote! { #krate::__private::Parser::Csv },
            "yaml" => quote! { #krate::__private::Parser::Yaml },
            other => {
                return Err(syn::Error::new_spanned(
                    field_ident,
                    format!("unknown kwconf parser {other:?}; expected auto, csv, or yaml"),
                ));
            }
        };

        let help = option_lit(opts.help.as_deref());
        let env = option_lit(opts.env.as_deref());
        let alias_lits = opts.aliases.iter().map(|value| quote! { #value });
        let choice_lits = opts.choices.iter().map(|value| quote! { #value });
        let kind = if opts.subconfig {
            quote! { #krate::__private::FieldKind::Subconfig(<#field_ty as #krate::Config>::config_spec()) }
        } else {
            quote! { #krate::__private::FieldKind::Value }
        };
        let value_type = if is_bool_type(&field_ty) {
            quote! { #krate::__private::ValueType::Bool }
        } else {
            quote! { #krate::__private::ValueType::Other }
        };

        infos.push(quote! {
            #krate::__private::FieldInfo {
                name: #field_name,
                aliases: &[#(#alias_lits),*],
                env: #env,
                help: #help,
                parser: #parser,
                choices: &[#(#choice_lits),*],
                kind: #kind,
                value_type: #value_type,
            }
        });
    }

    let spec_name = struct_opts.name.unwrap_or_else(|| ident.to_string());
    let spec_about = option_lit(struct_opts.about.as_deref());
    let special_options = struct_opts.special_options.to_tokens(&krate);

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let default_where = with_bounds(
        where_clause,
        &input.generics,
        quote! { ::core::default::Default },
    );
    let config_where = with_bounds(
        where_clause,
        &input.generics,
        quote! { ::core::default::Default + #krate::__private::serde::Serialize + #krate::__private::serde::de::DeserializeOwned },
    );

    Ok(quote! {
        impl #impl_generics ::core::default::Default for #ident #ty_generics #default_where {
            fn default() -> Self {
                Self {
                    #(#default_fields),*
                }
            }
        }

        impl #impl_generics #krate::Config for #ident #ty_generics #config_where {
            fn config_spec() -> &'static #krate::__private::ConfigSpec {
                static SPEC: ::std::sync::OnceLock<#krate::__private::ConfigSpec> = ::std::sync::OnceLock::new();
                SPEC.get_or_init(|| {
                    #krate::__private::ConfigSpec {
                        name: #spec_name,
                        about: #spec_about,
                        fields: ::std::vec![#(#infos),*],
                        special_options: #special_options,
                    }
                })
            }
        }

        impl #impl_generics #ident #ty_generics #config_where {
            pub fn from_sources(sources: #krate::Sources) -> #krate::Result<Self> {
                <Self as #krate::Config>::from_sources(sources)
            }

            #[allow(clippy::should_implement_trait)]
            pub fn from_iter<__I, __T>(args: __I) -> #krate::Result<Self>
            where
                __I: ::core::iter::IntoIterator<Item = __T>,
                __T: ::core::convert::Into<::std::ffi::OsString>,
            {
                <Self as #krate::Config>::from_iter(args)
            }

            pub fn try_cli() -> #krate::Result<Self> {
                <Self as #krate::Config>::try_cli()
            }

            pub fn cli() -> Self {
                <Self as #krate::Config>::cli()
            }

            pub fn help() -> ::std::string::String {
                <Self as #krate::Config>::help()
            }

            pub fn help_with_color(color: #krate::ColorChoice) -> ::std::string::String {
                <Self as #krate::Config>::help_with_color(color)
            }

            pub fn completion_script(shell: #krate::CompletionShell, bin_name: &str) -> ::std::string::String {
                <Self as #krate::Config>::completion_script(shell, bin_name)
            }
        }
    })
}

fn expand_modal_config(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ident = input.ident;
    let enum_opts = StructOpts::from_attrs(&input.attrs)?;
    let krate = enum_opts
        .krate
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(::kwconf));

    let variants = match input.data {
        Data::Enum(data) => data.variants,
        _ => {
            return Err(syn::Error::new_spanned(
                ident,
                "kwconf::ModalConfig only supports enums",
            ));
        }
    };

    let mut names = NameSpace::default();
    let mut variant_infos = Vec::new();
    let mut variant_arms = Vec::new();
    let mut default_variant = None::<String>;

    for variant in variants {
        let variant_ident = variant.ident;
        let opts = VariantOpts::from_attrs(&variant.attrs)?;
        let variant_name = opts
            .name
            .unwrap_or_else(|| to_kebab_case(&variant_ident.to_string()));
        if opts.default {
            if default_variant.is_some() {
                return Err(syn::Error::new_spanned(
                    variant_ident,
                    "only one modal variant can be marked #[kwconf(default)]",
                ));
            }
            default_variant = Some(variant_name.clone());
        }

        let owner = format!("variant `{variant_ident}`");
        names.claim(&variant_name, &owner, &variant_ident)?;
        for alias in &opts.aliases {
            names.claim(
                alias,
                &format!("alias `{alias}` of {owner}"),
                &variant_ident,
            )?;
        }

        let inner_ty = match variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                fields.unnamed.into_iter().next().unwrap().ty
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    variant_ident,
                    "kwconf::ModalConfig variants must be tuple variants with one Config payload",
                ));
            }
        };

        let help = option_lit(opts.help.as_deref());
        let alias_lits = opts.aliases.iter().map(|value| quote! { #value });
        let variant_name_lit = variant_name.clone();

        variant_infos.push(quote! {
            #krate::__private::ModalVariantInfo {
                name: #variant_name_lit,
                aliases: &[#(#alias_lits),*],
                help: #help,
                config_spec: <#inner_ty as #krate::Config>::config_spec(),
            }
        });

        variant_arms.push(quote! {
            #variant_name_lit => {
                #krate::__private::resolve_modal_variant::<#inner_ty>(selection).map(Self::#variant_ident)
            }
        });
    }

    let spec_name = enum_opts.name.unwrap_or_else(|| ident.to_string());
    let spec_about = option_lit(enum_opts.about.as_deref());
    let default_variant_tokens = option_lit(default_variant.as_deref());
    let special_options = enum_opts.special_options.to_tokens(&krate);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #krate::ModalConfig for #ident #ty_generics #where_clause {
            fn modal_spec() -> &'static #krate::__private::ModalSpec {
                static SPEC: ::std::sync::OnceLock<#krate::__private::ModalSpec> = ::std::sync::OnceLock::new();
                SPEC.get_or_init(|| {
                    #krate::__private::ModalSpec {
                        name: #spec_name,
                        about: #spec_about,
                        variants: ::std::vec![#(#variant_infos),*],
                        default_variant: #default_variant_tokens,
                        special_options: #special_options,
                    }
                })
            }

            fn from_sources(sources: #krate::Sources) -> #krate::Result<Self> {
                let selection = #krate::__private::resolve_modal_selection(Self::modal_spec(), sources)?;
                match selection.variant() {
                    #(#variant_arms),*,
                    other => ::core::result::Result::Err(#krate::Error::InvalidModalVariant(other.to_string())),
                }
            }
        }

        impl #impl_generics #ident #ty_generics #where_clause {
            pub fn from_sources(sources: #krate::Sources) -> #krate::Result<Self> {
                <Self as #krate::ModalConfig>::from_sources(sources)
            }

            #[allow(clippy::should_implement_trait)]
            pub fn from_iter<__I, __T>(args: __I) -> #krate::Result<Self>
            where
                __I: ::core::iter::IntoIterator<Item = __T>,
                __T: ::core::convert::Into<::std::ffi::OsString>,
            {
                <Self as #krate::ModalConfig>::from_iter(args)
            }

            pub fn try_cli() -> #krate::Result<Self> {
                <Self as #krate::ModalConfig>::try_cli()
            }

            pub fn cli() -> Self {
                <Self as #krate::ModalConfig>::cli()
            }

            pub fn help() -> ::std::string::String {
                <Self as #krate::ModalConfig>::help()
            }

            pub fn help_with_color(color: #krate::ColorChoice) -> ::std::string::String {
                <Self as #krate::ModalConfig>::help_with_color(color)
            }

            pub fn completion_script(shell: #krate::CompletionShell, bin_name: &str) -> ::std::string::String {
                <Self as #krate::ModalConfig>::completion_script(shell, bin_name)
            }
        }
    })
}

/// Extend a where clause with `bounds` for every generic type parameter.
fn with_bounds(
    where_clause: Option<&syn::WhereClause>,
    generics: &Generics,
    bounds: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let params: Vec<&syn::Ident> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(ty) => Some(&ty.ident),
            _ => None,
        })
        .collect();
    if params.is_empty() {
        return quote! { #where_clause };
    }
    let existing: Vec<&syn::WherePredicate> = where_clause
        .map(|clause| clause.predicates.iter().collect())
        .unwrap_or_default();
    quote! {
        where #(#existing,)* #(#params: #bounds),*
    }
}

fn mentions_generic(ty: &Type, generic_idents: &[String]) -> bool {
    if generic_idents.is_empty() {
        return false;
    }
    let rendered = ty.to_token_stream().to_string();
    rendered
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .any(|word| generic_idents.iter().any(|ident| ident == word))
}

fn option_lit(value: Option<&str>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => quote! { ::core::option::Option::Some(#value) },
        None => quote! { ::core::option::Option::None },
    }
}

fn default_expr_for_field(field_ty: &Type, expr: &Expr) -> proc_macro2::TokenStream {
    if is_string_type(field_ty) && is_string_literal(expr) {
        quote! { ::std::string::String::from(#expr) }
    } else {
        quote! { (|| -> #field_ty { #expr })() }
    }
}

fn is_named_type(field_ty: &Type, name: &str) -> bool {
    let Type::Path(path) = field_ty else {
        return false;
    };
    path.qself.is_none()
        && path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name && segment.arguments.is_empty())
}

fn is_string_type(field_ty: &Type) -> bool {
    is_named_type(field_ty, "String")
}

fn is_bool_type(field_ty: &Type) -> bool {
    is_named_type(field_ty, "bool")
}

fn is_string_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Lit(ExprLit {
            lit: Lit::Str(_),
            ..
        })
    )
}

#[derive(Default)]
struct SpecialOptionsOpts {
    config: bool,
    color: bool,
    completion: bool,
}

impl SpecialOptionsOpts {
    fn to_tokens(&self, krate: &syn::Path) -> proc_macro2::TokenStream {
        let config = self.config;
        let color = self.color;
        let completion = self.completion;
        quote! {
            #krate::__private::SpecialOptions {
                config: #config,
                color: #color,
                completion: #completion,
            }
        }
    }
}

#[derive(Default)]
struct StructOpts {
    name: Option<String>,
    about: Option<String>,
    special_options: SpecialOptionsOpts,
    krate: Option<syn::Path>,
}

impl StructOpts {
    fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut opts = StructOpts::default();
        for attr in attrs.iter().filter(|attr| attr.path().is_ident("kwconf")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    opts.name = Some(parse_lit_string(meta.value()?)?);
                    Ok(())
                } else if meta.path.is_ident("about") {
                    opts.about = Some(parse_lit_string(meta.value()?)?);
                    Ok(())
                } else if meta.path.is_ident("crate") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    opts.krate = Some(lit.parse::<syn::Path>()?);
                    Ok(())
                } else if meta.path.is_ident("special_options") {
                    meta.parse_nested_meta(|nested| {
                        if nested.path.is_ident("config") {
                            opts.special_options.config = true;
                            Ok(())
                        } else if nested.path.is_ident("color") {
                            opts.special_options.color = true;
                            Ok(())
                        } else if nested.path.is_ident("completion")
                            || nested.path.is_ident("completions")
                            || nested.path.is_ident("generate_completion")
                            || nested.path.is_ident("generate_completions")
                        {
                            opts.special_options.completion = true;
                            Ok(())
                        } else {
                            Err(nested.error("unsupported special option; expected config, color, or generate_completion"))
                        }
                    })
                } else {
                    Err(meta.error("unsupported kwconf attribute; expected name, about, crate, or special_options"))
                }
            })?;
        }
        Ok(opts)
    }
}

#[derive(Default)]
struct FieldOpts {
    default: Option<Option<Expr>>,
    help: Option<String>,
    parser: Option<String>,
    env: Option<String>,
    aliases: Vec<String>,
    choices: Vec<String>,
    subconfig: bool,
    modal: bool,
}

impl FieldOpts {
    fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut opts = FieldOpts::default();
        for attr in attrs.iter().filter(|attr| attr.path().is_ident("kwconf")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    if meta.input.peek(syn::Token![=]) {
                        let value = meta.value()?;
                        let expr: Expr = value.parse()?;
                        opts.default = Some(Some(expr));
                    } else {
                        opts.default = Some(None);
                    }
                    Ok(())
                } else if meta.path.is_ident("help") {
                    opts.help = Some(parse_lit_string(meta.value()?)?);
                    Ok(())
                } else if meta.path.is_ident("parser") {
                    opts.parser = Some(parse_lit_string(meta.value()?)?);
                    Ok(())
                } else if meta.path.is_ident("env") {
                    opts.env = Some(parse_lit_string(meta.value()?)?);
                    Ok(())
                } else if meta.path.is_ident("alias") {
                    opts.aliases.push(parse_lit_string(meta.value()?)?);
                    Ok(())
                } else if meta.path.is_ident("choices") {
                    let value = meta.value()?;
                    let arr: ExprArray = value.parse()?;
                    opts.choices = parse_string_array(arr)?;
                    Ok(())
                } else if meta.path.is_ident("subconfig") {
                    opts.subconfig = true;
                    Ok(())
                } else if meta.path.is_ident("modal") {
                    opts.modal = true;
                    Ok(())
                } else {
                    Err(meta.error("unsupported field kwconf attribute"))
                }
            })?;
        }
        Ok(opts)
    }
}

#[derive(Default)]
struct VariantOpts {
    name: Option<String>,
    help: Option<String>,
    aliases: Vec<String>,
    default: bool,
}

impl VariantOpts {
    fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut opts = VariantOpts::default();
        for attr in attrs.iter().filter(|attr| attr.path().is_ident("kwconf")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    opts.name = Some(parse_lit_string(meta.value()?)?);
                    Ok(())
                } else if meta.path.is_ident("help") {
                    opts.help = Some(parse_lit_string(meta.value()?)?);
                    Ok(())
                } else if meta.path.is_ident("alias") {
                    opts.aliases.push(parse_lit_string(meta.value()?)?);
                    Ok(())
                } else if meta.path.is_ident("default") {
                    opts.default = true;
                    Ok(())
                } else {
                    Err(meta.error("unsupported variant kwconf attribute"))
                }
            })?;
        }
        Ok(opts)
    }
}

fn parse_lit_string(input: syn::parse::ParseStream<'_>) -> syn::Result<String> {
    let lit: syn::LitStr = input.parse()?;
    Ok(lit.value())
}

fn parse_string_array(arr: ExprArray) -> syn::Result<Vec<String>> {
    let mut values = Vec::new();
    for elem in arr.elems {
        match elem {
            Expr::Lit(ExprLit {
                lit: Lit::Str(lit), ..
            }) => values.push(lit.value()),
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "choices must be string literals",
                ))
            }
        }
    }
    Ok(values)
}

fn to_kebab_case(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('-');
            }
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else if ch == '_' {
            out.push('-');
        } else {
            out.push(ch);
        }
    }
    out
}
