//! Derive macros for [`kwconf`](https://crates.io/crates/kwconf).
//!
//! The macros are intentionally thin: they describe fields and generate typed
//! setters. `clap` remains the only argv grammar/parser implementation.

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use std::collections::HashMap;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, ExprArray, ExprLit, Fields,
    GenericArgument, GenericParam, Generics, Lit, PathArguments, Type,
};

#[proc_macro_derive(Cli, attributes(kwconf))]
pub fn derive_cli(input: TokenStream) -> TokenStream {
    derive_struct(input, StructFlavor::Cli)
}

#[proc_macro_derive(Config, attributes(kwconf))]
pub fn derive_config(input: TokenStream) -> TokenStream {
    derive_struct(input, StructFlavor::Config)
}

#[proc_macro_derive(ModalCli, attributes(kwconf))]
pub fn derive_modal_cli(input: TokenStream) -> TokenStream {
    derive_modal(input, ModalFlavor::Cli)
}

#[proc_macro_derive(ModalConfig, attributes(kwconf))]
pub fn derive_modal_config(input: TokenStream) -> TokenStream {
    derive_modal(input, ModalFlavor::Config)
}

fn derive_struct(input: TokenStream, flavor: StructFlavor) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_struct(input, flavor) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_modal(input: TokenStream, flavor: ModalFlavor) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_modal(input, flavor) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructFlavor {
    Cli,
    Config,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModalFlavor {
    Cli,
    Config,
}

/// Dash/underscore-insensitive CLI name form.
fn normalize(name: &str) -> String {
    name.trim_start_matches('-').replace('-', "_")
}

/// Tracks one option namespace so collisions become compile errors where they
/// are statically knowable. Cross-subconfig collisions are validated by the
/// shared runtime command builder.
#[derive(Default)]
struct NameSpace {
    claimed: HashMap<String, String>,
}

impl NameSpace {
    fn claim(&mut self, name: &str, owner: &str, span: &dyn ToTokens) -> syn::Result<()> {
        let key = normalize(name);
        if key.is_empty() {
            return Err(syn::Error::new_spanned(span, "option names cannot be empty"));
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

fn expand_struct(
    input: DeriveInput,
    flavor: StructFlavor,
) -> syn::Result<proc_macro2::TokenStream> {
    let DeriveInput {
        ident,
        attrs,
        generics,
        data,
        ..
    } = input;
    let mut struct_opts = StructOpts::from_attrs(&attrs)?;
    if struct_opts.about.is_none() {
        struct_opts.about = doc_text(&attrs);
    }
    let krate = struct_opts
        .krate
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(::kwconf));

    if flavor == StructFlavor::Cli && struct_opts.special_options.config {
        return Err(syn::Error::new_spanned(
            &ident,
            "#[derive(kwconf::Cli)] cannot enable special_options(config); use kwconf::Config for layered config-file/env support",
        ));
    }

    let fields = match data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => {
                let message = match flavor {
                    StructFlavor::Cli => "kwconf::Cli only supports structs with named fields",
                    StructFlavor::Config => "kwconf::Config only supports structs with named fields",
                };
                return Err(syn::Error::new_spanned(ident, message));
            }
        },
        _ => {
            let message = match flavor {
                StructFlavor::Cli => "kwconf::Cli only supports structs",
                StructFlavor::Config => "kwconf::Config only supports structs",
            };
            return Err(syn::Error::new_spanned(ident, message));
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

    let generic_idents: Vec<String> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(ty) => Some(ty.ident.to_string()),
            _ => None,
        })
        .collect();

    let mut default_fields = Vec::new();
    let mut infos = Vec::new();
    let mut config_raw_arms = Vec::new();
    let mut config_value_arms = Vec::new();
    let mut cli_arms = Vec::new();

    for field in fields {
        let field_ident = field.ident.expect("named fields have names");
        let field_name = field_ident.to_string().trim_start_matches("r#").to_string();
        let field_ty = field.ty;
        let mut opts = FieldOpts::from_attrs(&field.attrs)?;
        if opts.help.is_none() {
            opts.help = doc_text(&field.attrs);
        }

        if opts.modal {
            return Err(syn::Error::new_spanned(
                &field_ident,
                "#[kwconf(modal)] is reserved; use ModalCli or ModalConfig on an enum",
            ));
        }
        if opts.subconfig && mentions_generic(&field_ty, &generic_idents) {
            return Err(syn::Error::new_spanned(
                &field_ty,
                "#[kwconf(subconfig)] fields cannot use the struct's generic parameters",
            ));
        }
        if flavor == StructFlavor::Cli && opts.env.is_some() {
            return Err(syn::Error::new_spanned(
                &field_ident,
                "#[kwconf(env = ...)] belongs to layered Config; the lightweight Cli API reads argv only",
            ));
        }

        let parser_name = opts.parser.as_deref().unwrap_or("auto");
        let parser = parser_tokens(parser_name, &krate, &field_ident)?;
        if flavor == StructFlavor::Cli {
            validate_cli_field(&field_ty, parser_name, opts.subconfig, &field_ident)?;
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

        let help = option_lit(opts.help.as_deref());
        let env = option_lit(opts.env.as_deref());
        let alias_lits = opts.aliases.iter().map(|value| quote! { #value });
        let choice_lits = opts.choices.iter().map(|value| quote! { #value });
        let kind = if opts.subconfig {
            match flavor {
                StructFlavor::Cli => {
                    quote! { #krate::__private::FieldKind::Subconfig(<#field_ty as #krate::Cli>::cli_spec()) }
                }
                StructFlavor::Config => {
                    quote! { #krate::__private::FieldKind::Subconfig(<#field_ty as #krate::Config>::config_spec()) }
                }
            }
        } else {
            quote! { #krate::__private::FieldKind::Value }
        };
        let value_type = if is_bool_type(&field_ty) && !opts.subconfig {
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

        match flavor {
            StructFlavor::Config => {
                if opts.subconfig {
                    config_raw_arms.push(quote! {
                        #field_name => <#field_ty as #krate::Config>::__kwconf_apply_raw(
                            &mut self.#field_ident,
                            rest,
                            full_name,
                            text,
                            source,
                        )
                    });
                    config_value_arms.push(quote! {
                        #field_name => <#field_ty as #krate::Config>::__kwconf_apply_value(
                            &mut self.#field_ident,
                            rest,
                            full_name,
                            value,
                            source,
                        )
                    });
                } else {
                    config_raw_arms.push(quote! {
                        #field_name if rest.is_empty() => {
                            self.#field_ident = #krate::__private::parse_config_raw::<#field_ty>(
                                full_name,
                                text,
                                #parser,
                                source,
                            )?;
                            ::core::result::Result::Ok(())
                        }
                    });
                    config_value_arms.push(quote! {
                        #field_name if rest.is_empty() => {
                            self.#field_ident = #krate::__private::parse_config_value::<#field_ty>(
                                full_name,
                                value,
                            )?;
                            ::core::result::Result::Ok(())
                        }
                    });
                }
            }
            StructFlavor::Cli => {
                if opts.subconfig {
                    cli_arms.push(quote! {
                        #field_name => <#field_ty as #krate::Cli>::__kwconf_apply_cli(
                            &mut self.#field_ident,
                            rest,
                            full_name,
                            text,
                        )
                    });
                } else {
                    let parse_expr = cli_parse_expr(&field_ty, parser_name, &krate)?;
                    cli_arms.push(quote! {
                        #field_name if rest.is_empty() => {
                            self.#field_ident = #parse_expr;
                            ::core::result::Result::Ok(())
                        }
                    });
                }
            }
        }
    }

    let spec_name = struct_opts.name.unwrap_or_else(|| ident.to_string());
    let spec_about = option_lit(struct_opts.about.as_deref());
    let special_options = struct_opts.special_options.to_tokens(&krate);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let default_where = with_bounds(
        where_clause,
        &generics,
        quote! { ::core::default::Default },
    );

    let default_impl = quote! {
        impl #impl_generics ::core::default::Default for #ident #ty_generics #default_where {
            fn default() -> Self {
                Self {
                    #(#default_fields),*
                }
            }
        }
    };

    let spec_expr = quote! {
        #krate::__private::ConfigSpec {
            name: #spec_name,
            about: #spec_about,
            fields: ::std::vec![#(#infos),*],
            special_options: #special_options,
        }
    };

    let expanded = match flavor {
        StructFlavor::Config => {
            let config_where = with_bounds(
                where_clause,
                &generics,
                quote! { ::core::default::Default + #krate::__private::serde::de::DeserializeOwned },
            );
            quote! {
                #default_impl

                impl #impl_generics #krate::Config for #ident #ty_generics #config_where {
                    fn config_spec() -> &'static #krate::__private::ConfigSpec {
                        static SPEC: ::std::sync::OnceLock<#krate::__private::ConfigSpec> = ::std::sync::OnceLock::new();
                        SPEC.get_or_init(|| #spec_expr)
                    }

                    fn __kwconf_apply_raw(
                        &mut self,
                        path: &[&'static #krate::__private::FieldInfo],
                        full_name: &str,
                        text: &str,
                        source: &'static str,
                    ) -> #krate::Result<()> {
                        let ::core::option::Option::Some((field, rest)) = path.split_first() else {
                            return ::core::result::Result::Err(#krate::Error::Schema(
                                "empty config field path".to_string(),
                            ));
                        };
                        match field.name {
                            #(#config_raw_arms,)*
                            _ => ::core::result::Result::Err(#krate::Error::Schema(
                                format!("invalid generated config field path {full_name:?}"),
                            )),
                        }
                    }

                    fn __kwconf_apply_value(
                        &mut self,
                        path: &[&'static #krate::__private::FieldInfo],
                        full_name: &str,
                        value: #krate::__private::serde_json::Value,
                        source: &'static str,
                    ) -> #krate::Result<()> {
                        let ::core::option::Option::Some((field, rest)) = path.split_first() else {
                            return ::core::result::Result::Err(#krate::Error::Schema(
                                "empty config field path".to_string(),
                            ));
                        };
                        match field.name {
                            #(#config_value_arms,)*
                            _ => ::core::result::Result::Err(#krate::Error::Schema(
                                format!("invalid generated config field path {full_name:?} from {source}"),
                            )),
                        }
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

                    pub fn try_completion_script(shell: #krate::CompletionShell, bin_name: &str) -> #krate::Result<::std::string::String> {
                        <Self as #krate::Config>::try_completion_script(shell, bin_name)
                    }

                    pub fn completion_script(shell: #krate::CompletionShell, bin_name: &str) -> ::std::string::String {
                        <Self as #krate::Config>::completion_script(shell, bin_name)
                    }
                }
            }
        }
        StructFlavor::Cli => {
            let cli_where = with_cli_bounds(where_clause, &generics);
            quote! {
                #default_impl

                impl #impl_generics #krate::Cli for #ident #ty_generics #cli_where {
                    fn cli_spec() -> &'static #krate::__private::ConfigSpec {
                        static SPEC: ::std::sync::OnceLock<#krate::__private::ConfigSpec> = ::std::sync::OnceLock::new();
                        SPEC.get_or_init(|| #spec_expr)
                    }

                    fn __kwconf_apply_cli(
                        &mut self,
                        path: &[&'static #krate::__private::FieldInfo],
                        full_name: &str,
                        text: &str,
                    ) -> #krate::Result<()> {
                        let ::core::option::Option::Some((field, rest)) = path.split_first() else {
                            return ::core::result::Result::Err(#krate::Error::Schema(
                                "empty CLI field path".to_string(),
                            ));
                        };
                        match field.name {
                            #(#cli_arms,)*
                            _ => ::core::result::Result::Err(#krate::Error::Schema(
                                format!("invalid generated CLI field path {full_name:?}"),
                            )),
                        }
                    }
                }

                impl #impl_generics #ident #ty_generics #cli_where {
                    #[allow(clippy::should_implement_trait)]
                    pub fn from_iter<__I, __T>(args: __I) -> #krate::Result<Self>
                    where
                        __I: ::core::iter::IntoIterator<Item = __T>,
                        __T: ::core::convert::Into<::std::ffi::OsString>,
                    {
                        <Self as #krate::Cli>::from_iter(args)
                    }

                    pub fn try_cli() -> #krate::Result<Self> {
                        <Self as #krate::Cli>::try_cli()
                    }

                    pub fn cli() -> Self {
                        <Self as #krate::Cli>::cli()
                    }

                    pub fn help() -> ::std::string::String {
                        <Self as #krate::Cli>::help()
                    }

                    pub fn help_with_color(color: #krate::ColorChoice) -> ::std::string::String {
                        <Self as #krate::Cli>::help_with_color(color)
                    }

                    pub fn try_completion_script(shell: #krate::CompletionShell, bin_name: &str) -> #krate::Result<::std::string::String> {
                        <Self as #krate::Cli>::try_completion_script(shell, bin_name)
                    }

                    pub fn completion_script(shell: #krate::CompletionShell, bin_name: &str) -> ::std::string::String {
                        <Self as #krate::Cli>::completion_script(shell, bin_name)
                    }
                }
            }
        }
    };

    Ok(expanded)
}

fn expand_modal(input: DeriveInput, flavor: ModalFlavor) -> syn::Result<proc_macro2::TokenStream> {
    let DeriveInput {
        ident,
        attrs,
        generics,
        data,
        ..
    } = input;
    let mut enum_opts = StructOpts::from_attrs(&attrs)?;
    if enum_opts.about.is_none() {
        enum_opts.about = doc_text(&attrs);
    }
    let krate = enum_opts
        .krate
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(::kwconf));

    if flavor == ModalFlavor::Cli && enum_opts.special_options.config {
        return Err(syn::Error::new_spanned(
            &ident,
            "#[derive(kwconf::ModalCli)] cannot enable special_options(config); use ModalConfig",
        ));
    }

    let variants = match data {
        Data::Enum(data) => data.variants,
        _ => {
            return Err(syn::Error::new_spanned(
                ident,
                "kwconf modal derives only support enums",
            ));
        }
    };

    let mut names = NameSpace::default();
    let mut variant_infos = Vec::new();
    let mut variant_arms = Vec::new();
    let mut default_variant = None::<String>;

    for variant in variants {
        let variant_ident = variant.ident;
        let mut opts = VariantOpts::from_attrs(&variant.attrs)?;
        if opts.help.is_none() {
            opts.help = doc_text(&variant.attrs);
        }
        let variant_name = opts
            .name
            .unwrap_or_else(|| to_kebab_case(&variant_ident.to_string()));
        if opts.default {
            if default_variant.is_some() {
                return Err(syn::Error::new_spanned(
                    &variant_ident,
                    "only one modal variant can be marked #[kwconf(default)]",
                ));
            }
            default_variant = Some(variant_name.clone());
        }

        let owner = format!("variant `{variant_ident}`");
        names.claim(&variant_name, &owner, &variant_ident)?;
        for alias in &opts.aliases {
            names.claim(alias, &format!("alias `{alias}` of {owner}"), &variant_ident)?;
        }

        let inner_ty = match variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                fields.unnamed.into_iter().next().unwrap().ty
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    &variant_ident,
                    "kwconf modal variants must be tuple variants with one payload",
                ));
            }
        };

        let help = option_lit(opts.help.as_deref());
        let alias_lits = opts.aliases.iter().map(|value| quote! { #value });
        let variant_name_lit = variant_name.clone();
        let payload_spec = match flavor {
            ModalFlavor::Cli => quote! { <#inner_ty as #krate::Cli>::cli_spec() },
            ModalFlavor::Config => quote! { <#inner_ty as #krate::Config>::config_spec() },
        };

        variant_infos.push(quote! {
            #krate::__private::ModalVariantInfo {
                name: #variant_name_lit,
                aliases: &[#(#alias_lits),*],
                help: #help,
                spec: #payload_spec,
            }
        });

        let resolver = match flavor {
            ModalFlavor::Cli => quote! { #krate::__private::resolve_modal_cli_variant::<#inner_ty>(selection) },
            ModalFlavor::Config => quote! { #krate::__private::resolve_modal_variant::<#inner_ty>(selection) },
        };
        variant_arms.push(quote! {
            #variant_name_lit => #resolver.map(Self::#variant_ident)
        });
    }

    let spec_name = enum_opts.name.unwrap_or_else(|| ident.to_string());
    let spec_about = option_lit(enum_opts.about.as_deref());
    let default_variant_tokens = option_lit(default_variant.as_deref());
    let special_options = enum_opts.special_options.to_tokens(&krate);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let spec_expr = quote! {
        #krate::__private::ModalSpec {
            name: #spec_name,
            about: #spec_about,
            variants: ::std::vec![#(#variant_infos),*],
            default_variant: #default_variant_tokens,
            special_options: #special_options,
        }
    };

    Ok(match flavor {
        ModalFlavor::Config => quote! {
            impl #impl_generics #krate::ModalConfig for #ident #ty_generics #where_clause {
                fn modal_spec() -> &'static #krate::__private::ModalSpec {
                    static SPEC: ::std::sync::OnceLock<#krate::__private::ModalSpec> = ::std::sync::OnceLock::new();
                    SPEC.get_or_init(|| #spec_expr)
                }

                fn from_sources(sources: #krate::Sources) -> #krate::Result<Self> {
                    let selection = #krate::__private::resolve_modal_selection(Self::modal_spec(), sources)?;
                    match selection.variant() {
                        #(#variant_arms,)*
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

                pub fn try_completion_script(shell: #krate::CompletionShell, bin_name: &str) -> #krate::Result<::std::string::String> {
                    <Self as #krate::ModalConfig>::try_completion_script(shell, bin_name)
                }

                pub fn completion_script(shell: #krate::CompletionShell, bin_name: &str) -> ::std::string::String {
                    <Self as #krate::ModalConfig>::completion_script(shell, bin_name)
                }
            }
        },
        ModalFlavor::Cli => quote! {
            impl #impl_generics #krate::ModalCli for #ident #ty_generics #where_clause {
                fn modal_cli_spec() -> &'static #krate::__private::ModalSpec {
                    static SPEC: ::std::sync::OnceLock<#krate::__private::ModalSpec> = ::std::sync::OnceLock::new();
                    SPEC.get_or_init(|| #spec_expr)
                }

                fn from_iter<__I, __T>(args: __I) -> #krate::Result<Self>
                where
                    __I: ::core::iter::IntoIterator<Item = __T>,
                    __T: ::core::convert::Into<::std::ffi::OsString>,
                {
                    let selection = #krate::__private::resolve_modal_cli_selection(Self::modal_cli_spec(), args)?;
                    match selection.variant() {
                        #(#variant_arms,)*
                        other => ::core::result::Result::Err(#krate::Error::InvalidModalVariant(other.to_string())),
                    }
                }
            }

            impl #impl_generics #ident #ty_generics #where_clause {
                #[allow(clippy::should_implement_trait)]
                pub fn from_iter<__I, __T>(args: __I) -> #krate::Result<Self>
                where
                    __I: ::core::iter::IntoIterator<Item = __T>,
                    __T: ::core::convert::Into<::std::ffi::OsString>,
                {
                    <Self as #krate::ModalCli>::from_iter(args)
                }

                pub fn try_cli() -> #krate::Result<Self> {
                    <Self as #krate::ModalCli>::try_cli()
                }

                pub fn cli() -> Self {
                    <Self as #krate::ModalCli>::cli()
                }

                pub fn help() -> ::std::string::String {
                    <Self as #krate::ModalCli>::help()
                }

                pub fn help_with_color(color: #krate::ColorChoice) -> ::std::string::String {
                    <Self as #krate::ModalCli>::help_with_color(color)
                }

                pub fn try_completion_script(shell: #krate::CompletionShell, bin_name: &str) -> #krate::Result<::std::string::String> {
                    <Self as #krate::ModalCli>::try_completion_script(shell, bin_name)
                }

                pub fn completion_script(shell: #krate::CompletionShell, bin_name: &str) -> ::std::string::String {
                    <Self as #krate::ModalCli>::completion_script(shell, bin_name)
                }
            }
        },
    })
}

fn parser_tokens(
    parser: &str,
    krate: &syn::Path,
    span: &dyn ToTokens,
) -> syn::Result<proc_macro2::TokenStream> {
    match parser {
        "auto" => Ok(quote! { #krate::__private::Parser::Auto }),
        "csv" => Ok(quote! { #krate::__private::Parser::Csv }),
        "yaml" => Ok(quote! { #krate::__private::Parser::Yaml }),
        other => Err(syn::Error::new_spanned(
            span,
            format!("unknown kwconf parser {other:?}; expected auto, csv, or yaml"),
        )),
    }
}

fn validate_cli_field(
    ty: &Type,
    parser: &str,
    subconfig: bool,
    span: &dyn ToTokens,
) -> syn::Result<()> {
    if subconfig {
        if parser != "auto" {
            return Err(syn::Error::new_spanned(
                span,
                "subconfig fields do not take a parser",
            ));
        }
        return Ok(());
    }
    if parser == "yaml" {
        return Err(syn::Error::new_spanned(
            span,
            "parser = \"yaml\" requires layered Config support; lightweight Cli intentionally has no Serde dependency",
        ));
    }
    let unwrapped = option_inner(ty).unwrap_or(ty);
    let is_vec = vec_inner(unwrapped).is_some();
    match (parser, is_vec) {
        ("csv", false) => Err(syn::Error::new_spanned(
            span,
            "parser = \"csv\" requires Vec<T> or Option<Vec<T>> in lightweight Cli",
        )),
        ("auto", true) => Err(syn::Error::new_spanned(
            span,
            "Vec<T> fields in lightweight Cli require parser = \"csv\"; use Config for structured JSON/YAML collection parsing",
        )),
        _ => Ok(()),
    }
}

fn cli_parse_expr(
    ty: &Type,
    parser: &str,
    krate: &syn::Path,
) -> syn::Result<proc_macro2::TokenStream> {
    if let Some(inner) = option_inner(ty) {
        if let Some(elem) = vec_inner(inner) {
            debug_assert_eq!(parser, "csv");
            return Ok(quote! {
                #krate::__private::parse_cli_optional_csv::<#elem>(full_name, text)?
            });
        }
        if is_bool_type(inner) {
            return Ok(quote! {
                if text.trim().eq_ignore_ascii_case("none") || text.trim().eq_ignore_ascii_case("null") {
                    ::core::option::Option::None
                } else {
                    ::core::option::Option::Some(#krate::__private::parse_cli_bool(full_name, text)?)
                }
            });
        }
        return Ok(quote! {
            #krate::__private::parse_cli_optional::<#inner>(full_name, text)?
        });
    }
    if let Some(elem) = vec_inner(ty) {
        debug_assert_eq!(parser, "csv");
        return Ok(quote! {
            #krate::__private::parse_cli_csv::<#elem>(full_name, text)?
        });
    }
    if is_bool_type(ty) {
        return Ok(quote! { #krate::__private::parse_cli_bool(full_name, text)? });
    }
    Ok(quote! { #krate::__private::parse_cli_value::<#ty>(full_name, text)? })
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

fn with_cli_bounds(
    where_clause: Option<&syn::WhereClause>,
    generics: &Generics,
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
        where
            #(#existing,)*
            #(#params: ::core::default::Default + ::core::str::FromStr,)*
            #(<#params as ::core::str::FromStr>::Err: ::core::fmt::Display,)*
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

fn single_type_arg<'a>(field_ty: &'a Type, name: &str) -> Option<&'a Type> {
    let Type::Path(path) = field_ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != name {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.first()? {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    }
}

fn option_inner(ty: &Type) -> Option<&Type> {
    single_type_arg(ty, "Option")
}

fn vec_inner(ty: &Type) -> Option<&Type> {
    single_type_arg(ty, "Vec")
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

fn doc_text(attrs: &[Attribute]) -> Option<String> {
    let parts: Vec<String> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(meta) => match &meta.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(text),
                    ..
                }) => Some(text.value().trim().to_string()),
                _ => None,
            },
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
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
                            Err(nested.error(
                                "unsupported special option; expected config, color, or generate_completion",
                            ))
                        }
                    })
                } else {
                    Err(meta.error(
                        "unsupported kwconf attribute; expected name, about, crate, or special_options",
                    ))
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
                lit: Lit::Str(lit),
                ..
            }) => values.push(lit.value()),
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "choices must be string literals",
                ));
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
