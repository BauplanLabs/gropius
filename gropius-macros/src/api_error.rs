use proc_macro2::TokenStream;
use proc_macro2_diagnostics::{Diagnostic, SpanDiagnosticExt};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Attribute, Data, DeriveInput, Fields};

pub(crate) fn expand(input: DeriveInput) -> TokenStream {
    let mut errors: Vec<Diagnostic> = Vec::new();

    let body = match &input.data {
        Data::Struct(_) => match parse_status(&input.attrs) {
            Ok(Some(Status::Transparent(span))) => {
                errors.push(span.error("`#[api_error(transparent)]` requires a newtype variant"));
                TokenStream::new()
            }
            Ok(Some(Status::Fixed(fixed))) => status_expr(&fixed),
            Ok(None) => {
                errors.push(
                    input
                        .ident
                        .span()
                        .error("missing `#[api_error(...)]` attribute"),
                );
                TokenStream::new()
            }
            Err(diag) => {
                errors.push(diag);
                TokenStream::new()
            }
        },
        Data::Enum(data) => {
            if let Some(attr) = input.attrs.iter().find(|a| a.path().is_ident("api_error")) {
                errors.push(
                    attr.span()
                        .error("put `#[api_error(...)]` on each variant, not on the enum"),
                );
            }

            if data.variants.is_empty() {
                errors.push(
                    input
                        .ident
                        .span()
                        .error("cannot derive ApiError for an empty enum")
                        .help("use `std::convert::Infallible` instead"),
                );
            }

            let arms =
                data.variants.iter().filter_map(|variant| {
                    let status = match parse_status(&variant.attrs) {
                        Ok(Some(status)) => status,
                        Ok(None) => {
                            errors.push(
                                variant
                                    .ident
                                    .span()
                                    .error("missing `#[api_error(...)]` attribute"),
                            );
                            return None;
                        }
                        Err(diag) => {
                            errors.push(diag);
                            return None;
                        }
                    };

                    let ident = &variant.ident;
                    match status {
                        Status::Transparent(span) => match &variant.fields {
                            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Some(quote! {
                                Self::#ident(inner) => ::gropius::ApiError::status_code(inner)
                            }),
                            _ => {
                                errors.push(span.error(
                                    "`#[api_error(transparent)]` requires a newtype variant",
                                ));
                                None
                            }
                        },
                        Status::Fixed(fixed) => {
                            let pattern = match &variant.fields {
                                Fields::Unit => quote! { Self::#ident },
                                Fields::Unnamed(_) => quote! { Self::#ident(..) },
                                Fields::Named(_) => quote! { Self::#ident { .. } },
                            };

                            let status = status_expr(&fixed);
                            Some(quote! { #pattern => #status })
                        }
                    }
                });

            quote! { match self { #(#arms,)* } }
        }
        Data::Union(_) => {
            errors.push(
                input
                    .ident
                    .span()
                    .error("ApiError can only be derived for structs and enums"),
            );
            TokenStream::new()
        }
    };

    if !errors.is_empty() {
        let error_tokens = errors.into_iter().map(|d| d.emit_as_item_tokens());
        return quote! { #(#error_tokens)* };
    }

    let name = &input.ident;
    let (_, ty_generics, _) = input.generics.split_for_impl();

    // Serialize and JsonSchema derives emit bounds conditional on the type
    // parameters, so the supertraits are deferred to the use site.
    let mut generics = input.generics.clone();
    if !generics.params.is_empty() {
        generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote! {
                #name #ty_generics: ::gropius::generated::serde::Serialize
                    + ::gropius::generated::schemars::JsonSchema
            });
    }
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics ::gropius::ApiError for #name #ty_generics #where_clause {
            fn status_code(&self) -> ::gropius::generated::http::StatusCode {
                #body
            }
        }
    }
}

fn status_expr(status: &Fixed) -> TokenStream {
    // The const block makes an invalid code unrepresentable at runtime:
    // from_u16 is const, and a named constant must already be a StatusCode.
    match status {
        Fixed::Code(code) => quote! {
            const {
                match ::gropius::generated::http::StatusCode::from_u16(#code) {
                    Ok(status) => status,
                    Err(_) => panic!("status code is range-checked at derive time"),
                }
            }
        },
        Fixed::Const(path) => quote! { const { #path } },
    }
}

/// A parsed `#[api_error(...)]` attribute.
enum Status {
    /// A fixed status.
    Fixed(Fixed),
    /// Delegate to the inner error of a newtype variant, from `transparent`.
    Transparent(proc_macro2::Span),
}

/// A fixed status argument.
enum Fixed {
    /// A bare code, like `404`.
    Code(u16),
    /// A named constant, like `http::StatusCode::NOT_FOUND`.
    Const(syn::Path),
}

/// Parses an `#[api_error(...)]` attribute, taking an integer literal
/// (range-checked here), a path to a `StatusCode` constant, or `transparent`.
fn parse_status(attrs: &[Attribute]) -> Result<Option<Status>, Diagnostic> {
    let mut found = None;
    for attr in attrs {
        if !attr.path().is_ident("api_error") {
            continue;
        }

        if found.is_some() {
            return Err(attr.span().error("duplicate `#[api_error(...)]` attribute"));
        }

        found = Some(
            attr.parse_args_with(parse_status_args)
                .map_err(|e| e.span().error(e.to_string()))?,
        );
    }

    Ok(found)
}

fn parse_status_args(input: syn::parse::ParseStream<'_>) -> syn::Result<Status> {
    const EXPECTED: &str = "expected a status code or `transparent`";

    let status = if input.peek(syn::LitInt) {
        let lit: syn::LitInt = input.parse()?;
        let code: u16 = lit
            .base10_parse()
            .map_err(|_| syn::Error::new(lit.span(), "expected an HTTP status code"))?;
        if !(100..=999).contains(&code) {
            return Err(syn::Error::new(
                lit.span(),
                "status code must be in the 100-999 range accepted by `http::StatusCode`",
            ));
        }
        Status::Fixed(Fixed::Code(code))
    } else {
        let path: syn::Path = input
            .parse()
            .map_err(|e| syn::Error::new(e.span(), EXPECTED))?;
        if path.is_ident("transparent") {
            Status::Transparent(path.span())
        } else {
            Status::Fixed(Fixed::Const(path))
        }
    };

    if !input.is_empty() {
        return Err(input.error(EXPECTED));
    }
    Ok(status)
}
