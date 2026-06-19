use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Expr, ExprArray, ExprLit, Fields, Lit, Type};

#[proc_macro_derive(Config, attributes(kwconf))]
pub fn derive_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_config(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_config(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ident = input.ident;
    let struct_opts = StructOpts::from_attrs(&input.attrs)?;

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

    let mut default_fields = Vec::new();
    let mut infos = Vec::new();

    for field in fields {
        let field_ident = field.ident.expect("named fields have names");
        let field_name = field_ident.to_string().trim_start_matches("r#").to_string();
        let field_ty = field.ty;
        let opts = FieldOpts::from_attrs(&field.attrs)?;

        let default_expr = match opts.default {
            Some(Some(expr)) => default_expr_for_field(&field_ty, &expr),
            Some(None) | None => quote! { <#field_ty as ::core::default::Default>::default() },
        };
        default_fields.push(quote! { #field_ident: #default_expr });

        let parser = match opts.parser.as_deref().unwrap_or("auto") {
            "auto" => quote! { ::kwconf::Parser::Auto },
            "csv" => quote! { ::kwconf::Parser::Csv },
            "yaml" => quote! { ::kwconf::Parser::Yaml },
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

        infos.push(quote! {
            ::kwconf::FieldInfo {
                name: #field_name,
                aliases: &[#(#alias_lits),*],
                env: #env,
                help: #help,
                parser: #parser,
                choices: &[#(#choice_lits),*],
            }
        });
    }

    let spec_name = struct_opts.name.unwrap_or_else(|| ident.to_string());
    let spec_about = option_lit(struct_opts.about.as_deref());
    Ok(quote! {
        impl ::core::default::Default for #ident {
            fn default() -> Self {
                Self {
                    #(#default_fields),*
                }
            }
        }

        impl ::kwconf::Config for #ident {
            fn config_spec() -> &'static ::kwconf::ConfigSpec {
                static SPEC: ::kwconf::ConfigSpec = ::kwconf::ConfigSpec {
                    name: #spec_name,
                    about: #spec_about,
                    fields: &[
                        #(#infos),*
                    ],
                };
                &SPEC
            }
        }

        impl #ident {
            pub fn from_sources(sources: ::kwconf::Sources) -> ::kwconf::Result<Self> {
                <Self as ::kwconf::Config>::from_sources(sources)
            }

            #[allow(clippy::should_implement_trait)]
            pub fn from_iter<I, T>(args: I) -> ::kwconf::Result<Self>
            where
                I: ::core::iter::IntoIterator<Item = T>,
                T: ::core::convert::Into<::std::ffi::OsString>,
            {
                <Self as ::kwconf::Config>::from_iter(args)
            }

            pub fn try_cli() -> ::kwconf::Result<Self> {
                <Self as ::kwconf::Config>::try_cli()
            }

            pub fn cli() -> Self {
                <Self as ::kwconf::Config>::cli()
            }

            pub fn help() -> ::std::string::String {
                <Self as ::kwconf::Config>::help()
            }

            pub fn help_with_color(color: ::kwconf::ColorChoice) -> ::std::string::String {
                <Self as ::kwconf::Config>::help_with_color(color)
            }

            pub fn completion_script(shell: ::kwconf::CompletionShell, bin_name: &str) -> ::std::string::String {
                <Self as ::kwconf::Config>::completion_script(shell, bin_name)
            }
        }
    })
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

fn is_string_type(field_ty: &Type) -> bool {
    let Type::Path(path) = field_ty else {
        return false;
    };
    path.qself.is_none()
        && path
            .path
            .segments
            .last()
            .map_or(false, |segment| segment.ident == "String" && segment.arguments.is_empty())
}

fn is_string_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(ExprLit { lit: Lit::Str(_), .. }))
}

#[derive(Default)]
struct StructOpts {
    name: Option<String>,
    about: Option<String>,
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
                } else {
                    Err(meta.error("unsupported struct kwconf attribute"))
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
                } else {
                    Err(meta.error("unsupported field kwconf attribute"))
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
            Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => values.push(lit.value()),
            other => return Err(syn::Error::new_spanned(other, "choices must be string literals")),
        }
    }
    Ok(values)
}
