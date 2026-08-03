use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use crate::api::{RawEndpoint, RequestKind, ResponseKind, path};

/// Arguments to `client(async, cfg(...))`.
#[derive(Default)]
pub(super) struct ClientAttr {
    is_async: bool,
    cfg: Option<TokenStream>,
}

impl syn::parse::Parse for ClientAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut is_async = false;
        let mut cfg = None;

        while !input.is_empty() {
            if input.peek(syn::Token![async]) {
                let _: syn::Token![async] = input.parse()?;
                is_async = true;
            } else {
                let key: syn::Ident = input.parse()?;
                if key == "cfg" {
                    let content;
                    syn::parenthesized!(content in input);
                    cfg = Some(content.parse()?);
                } else {
                    return Err(syn::Error::new_spanned(key, "unknown attribute"));
                }
            }

            let _ = input.parse::<syn::Token![,]>();
        }

        Ok(ClientAttr { is_async, cfg })
    }
}

pub(super) fn generate_client(
    client: &ClientAttr,
    vis: &syn::Visibility,
    trait_ident: &syn::Ident,
    trait_name: &str,
    endpoints: &[RawEndpoint],
) -> TokenStream {
    let client_ident = format_ident!("{}Client", trait_ident);
    let is_async = client.is_async;

    let inner_ty = if is_async {
        quote! { ::gropius::generated::client::AsyncClient }
    } else {
        quote! { ::gropius::generated::client::BlockingClient }
    };

    let transport_ty = if is_async {
        quote! { ::gropius::generated::client::AsyncTransport }
    } else {
        quote! { ::gropius::generated::client::BlockingTransport }
    };

    let cfg_attr = match &client.cfg {
        Some(cfg) => quote! { #[cfg(#cfg)] },
        None => quote! {},
    };

    let assertions = endpoints.iter().map(client_assertions);
    let methods = endpoints.iter().map(|ep| client_method(ep, vis, is_async));

    let struct_doc = format!("A generated client for [`{trait_name}`].");

    quote! {
        #cfg_attr
        const _: fn() = || {
            #(#assertions)*
        };

        #cfg_attr
        #[doc = #struct_doc]
        #[derive(Debug, Clone)]
        #vis struct #client_ident {
            inner: #inner_ty,
        }

        #cfg_attr
        impl #client_ident {
            /// Create a client for the given base URL.
            #vis fn new(base_url: impl ::core::convert::Into<::std::string::String>) -> Self {
                Self { inner: <#inner_ty>::new(base_url) }
            }

            /// Create a client for the given base URL from an existing transport.
            ///
            /// # `reqwest`
            ///
            /// This method accepts a `reqwest::Client` (for async) or a
            /// `reqwest::blocking::Client` (for sync).
            ///
            /// # `ureq`
            ///
            /// This method accepts a `ureq::Agent`. Note that the agent *must*
            /// have `http_status_as_error(false)` set in its config, or all
            /// errors will be [`TransportError`](crate::client::ClientError):
            ///
            /// ```rust
            ///  let transport: ureq::Agent = ureq::Agent::config_builder()
            ///    .http_status_as_error(false)
            ///    .build()
            ///    .into();
            /// ```
            #vis fn with_transport(
                base_url: impl ::core::convert::Into<::std::string::String>,
                transport: #transport_ty,
            ) -> Self {
                Self { inner: <#inner_ty>::with_transport(base_url, transport) }
            }

            #(#methods)*
        }
    }
}

/// Generates one client method for an endpoint.
fn client_method(ep: &RawEndpoint, vis: &syn::Visibility, is_async: bool) -> TokenStream {
    let span = ep.span;
    let name = &ep.name;
    let method = &ep.method;
    let path = &ep.path;
    let error_type = &ep.error_type;

    // NB: we ignore ep.raw_request, because there's no real way to handle it.
    let mut params = Vec::new();
    if let Some(ty) = &ep.path_type {
        params.push(quote! { path: #ty });
    }
    if let Some(ty) = &ep.query_type {
        params.push(quote! { query: #ty });
    }
    match &ep.request_type {
        Some(RequestKind::Json(ty)) => {
            params.push(quote! { body: #ty });
        }
        Some(RequestKind::Multipart(_)) => {
            params.push(quote! { boundary: &str });
            params.push(quote! {
                parts: impl ::core::iter::IntoIterator<
                    Item = ::gropius::generated::client::MultipartPart,
                >
            });
        }
        None => (),
    }

    let base_uri = if ep.path_type.is_some() {
        let template = path.value();
        let names = path::parameter_names(&template);
        if names.is_empty() {
            quote! {{
                ::gropius::generated::client::path_params(&path)?;
                ::std::string::String::from(#path)
            }}
        } else {
            // This boils down to:
            //
            // ```
            // format!(
            //     "/{foo}/{bar}",
            //     foo = segment(params, "foo", 0),
            //     bar = segment(params, "bar", 1)
            // )
            // ```
            let args = names.iter().enumerate().map(|(index, &name)| {
                let ident = path::param_ident(name)
                    .expect("invalid parameter names are rejected during validation");
                quote! {
                    #ident = ::gropius::generated::client::segment(&__params, #name, #index)?
                }
            });

            quote! {{
                let __params = ::gropius::generated::client::path_params(&path)?;
                ::std::format!(#path, #(#args),*)
            }}
        }
    } else {
        quote! { ::std::string::String::from(#path) }
    };

    let uri_stmts = if ep.query_type.is_some() {
        quote! {
            let mut __uri = #base_uri;
            ::gropius::generated::client::append_query(&mut __uri, &query)?;
        }
    } else {
        quote! { let __uri = #base_uri; }
    };

    let body_expr = match &ep.request_type {
        Some(RequestKind::Json(_)) => quote! {
            ::core::option::Option::Some((
                ::std::string::String::from("application/json"),
                ::gropius::generated::client::encode_body(&body)?,
            ))
        },
        Some(RequestKind::Multipart(_)) => quote! {
            ::core::option::Option::Some(::gropius::generated::client::encode_multipart(
                boundary, parts,
            ))
        },
        None => quote! { ::core::option::Option::None },
    };

    let (ok_type, parse_fn) = match &ep.response_kind {
        ResponseKind::Json(ty) => (
            quote! { #ty },
            quote! { ::gropius::generated::client::parse_json },
        ),
        ResponseKind::Empty => (
            quote! { () },
            quote! { ::gropius::generated::client::parse_empty },
        ),
        ResponseKind::Raw => (
            quote! { ::gropius::Response },
            quote! { ::gropius::generated::client::parse_raw },
        ),
    };

    // Infallible endpoints have no deserializable error type, so any error
    // status becomes `ClientError::Unexpected`.
    let error_parser = if ep.infallible {
        quote! { |_| ::core::option::Option::<#error_type>::None }
    } else {
        quote! {
            |__body| ::gropius::generated::client::serde_json::from_slice::<#error_type>(
                ::core::convert::AsRef::<[u8]>::as_ref(__body),
            )
            .ok()
        }
    };

    let run = if is_async {
        quote! {
            self.inner
                .run(::gropius::generated::client::http::Method::#method, &__uri, #body_expr)
                .await?
        }
    } else {
        quote! {
            self.inner
                .run(::gropius::generated::client::http::Method::#method, &__uri, #body_expr)?
        }
    };

    let asyncness = if is_async {
        quote! { async }
    } else {
        quote! {}
    };
    let doc_attr = match &ep.doc {
        Some(doc) => quote! { #[doc = #doc] },
        None => quote! {},
    };

    quote_spanned! { span =>
        #doc_attr
        #vis #asyncness fn #name(
            &self,
            #(#params),*
        ) -> ::core::result::Result<#ok_type, ::gropius::generated::client::ClientError<#error_type>> {
            #uri_stmts
            let __resp = #run;
            #parse_fn(__resp, #error_parser)
        }
    }
}

/// Type assertions for the client direction: request types must be
/// serializable, and response and error types must be deserializable.
fn client_assertions(ep: &RawEndpoint) -> TokenStream {
    let mut checks = Vec::new();

    for ty in [
        ep.path_type.as_ref(),
        ep.query_type.as_ref(),
        ep.request_type.as_ref().and_then(RequestKind::json_type),
    ]
    .into_iter()
    .flatten()
    {
        checks.push(quote_spanned! { ty.span() => {
            fn check<T: ::gropius::generated::serde::Serialize>() {}
            check::<#ty>();
        }});
    }

    if let ResponseKind::Json(ty) = &ep.response_kind {
        checks.push(quote_spanned! { ty.span() => {
            fn check<T: ::gropius::generated::serde::de::DeserializeOwned>() {}
            check::<#ty>();
        }});
    }

    if !ep.infallible {
        let error_type = &ep.error_type;
        checks.push(quote_spanned! { error_type.span() => {
            fn check<T: ::gropius::generated::serde::de::DeserializeOwned>() {}
            check::<#error_type>();
        }});
    }

    quote! { #(#checks)* }
}
