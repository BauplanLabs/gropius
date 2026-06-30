use std::ops::Deref;

use heck::ToShoutySnakeCase;
use proc_macro2::{Span, TokenStream};
use proc_macro2_diagnostics::{Diagnostic, SpanDiagnosticExt};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    FnArg, GenericArgument, ItemTrait, PatType, PathArguments, ReturnType, TraitItem, TraitItemFn,
    Type, TypeParamBound,
};

mod path;

/// Arguments to `#[gropius::api(tags = "...")]`.
struct ApiAttr {
    tags: Vec<syn::LitStr>,
}

impl syn::parse::Parse for ApiAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut tags = Vec::new();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            if key != "tags" {
                return Err(syn::Error::new_spanned(key, "unknown attribute"));
            }

            let _eq: syn::Token![=] = input.parse()?;
            let content;
            syn::bracketed!(content in input);
            let items =
                syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated(
                    &content,
                )?;
            tags.extend(items);

            let _ = input.parse::<syn::Token![,]>();
        }

        Ok(ApiAttr { tags })
    }
}

/// Arguments to `#[endpoint(GET, "/foo")]`.
struct EndpointAttr {
    method: syn::Ident,
    path: syn::LitStr,
    content_type: Option<syn::LitStr>,
}

impl syn::parse::Parse for EndpointAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let method: syn::Ident = input.parse()?;
        let _comma: syn::Token![,] = input.parse()?;
        let path: syn::LitStr = input.parse()?;

        let mut content_type = None;
        while !input.is_empty() {
            let _comma: syn::Token![,] = input.parse()?;
            if input.is_empty() {
                break;
            }
            let key: syn::Ident = input.parse()?;
            if key == "content_type" {
                let _eq: syn::Token![=] = input.parse()?;
                content_type = Some(input.parse()?);
            } else {
                return Err(syn::Error::new_spanned(key, "unknown attribute"));
            }
        }

        Ok(EndpointAttr {
            method,
            path,
            content_type,
        })
    }
}

/// The kind of response an endpoint produces.
enum ResponseKind {
    Json(Box<Type>),
    Empty,
    Raw,
}

/// Parsed endpoint method, with annotation.
struct RawEndpoint {
    span: Span,
    name: syn::Ident,
    method: syn::Ident,
    path: syn::LitStr,
    doc: Option<String>,
    path_type: Option<Type>,
    query_type: Option<Type>,
    request_type: Option<Type>,
    raw_request: bool,
    response_kind: ResponseKind,
    content_type: Option<syn::LitStr>,
    error_type: Type,
}

pub(crate) fn expand(attr: TokenStream, mut item_trait: ItemTrait) -> TokenStream {
    let mut errors: Vec<Diagnostic> = Vec::new();

    let tags = match syn::parse2::<ApiAttr>(attr) {
        Ok(attr) => attr.tags,
        Err(err) => {
            errors.push(err.span().error(err.to_string()));
            Vec::new()
        }
    };

    let trait_doc = extract_doc(&item_trait.attrs);

    let mut endpoints = Vec::new();
    for item in &mut item_trait.items {
        let method = match item {
            TraitItem::Fn(method) => method,
            other => {
                errors.push(other.span().error("unexpected trait item"));
                continue;
            }
        };

        match parse_endpoint(method) {
            Ok(ep) => endpoints.push(ep),
            Err(diag) => errors.push(diag),
        }
    }

    path::validate(&endpoints, &mut errors);

    let desc_tokens = endpoints.iter().map(|ep| {
        let name_str = ep.name.to_string();
        let method = &ep.method;
        let path = &ep.path;
        let path_str = ep.path.value();
        let path_param_names = path::parameter_names(&path_str);
        let doc_expr = quote_option(ep.doc.as_ref());
        let raw_request = ep.raw_request;
        let span = ep.span;
        let path_schema = schema_fn(ep.path_type.as_ref(), true, span);
        let query_schema = schema_fn(ep.query_type.as_ref(), true, span);
        let request_schema = schema_fn(ep.request_type.as_ref(), true, span);
        let error_type = &ep.error_type;

        let response_schema = match &ep.response_kind {
            ResponseKind::Json(ty) => quote_spanned! { span =>
                ::gropius::generated::ResponseType::Json(
                    |g: &mut ::schemars::SchemaGenerator| g.subschema_for::<#ty>()
                )
            },
            ResponseKind::Empty => quote! { ::gropius::generated::ResponseType::Empty },
            ResponseKind::Raw => {
                let ct = quote_option(ep.content_type.as_ref());
                quote! { ::gropius::generated::ResponseType::Raw(#ct) }
            }
        };

        let error_schema = quote_spanned! { span =>
            |g: &mut ::schemars::SchemaGenerator| g.subschema_for::<#error_type>()
        };

        quote! {
            ::gropius::generated::Endpoint {
                method: &::http::Method::#method,
                path: #path,
                path_params: &[#(#path_param_names),*],
                name: #name_str,
                doc: #doc_expr,
                raw_request: #raw_request,
                path_type: #path_schema,
                query_type: #query_schema,
                request_type: #request_schema,
                response_type: #response_schema,
                error_type: #error_schema,
            }
        }
    });

    let handler_tokens = endpoints.iter().map(|ep| {
        let span = ep.span;
        let name = &ep.name;
        let mut extractions = Vec::new();
        let mut args = Vec::new();

        if let Some(ty) = &ep.path_type {
            extractions.push(quote_spanned! { span =>
                let path = match ::gropius::Path::<#ty>::extract(_path_params) {
                    Ok(v) => v,
                    Err(e) => return ::std::boxed::Box::pin(::core::future::ready(Err(e))),
                };
            });

            args.push(quote! { path });
        }

        if let Some(ty) = &ep.query_type {
            extractions.push(quote_spanned! { span =>
                let query = match ::gropius::Query::<#ty>::extract(_req) {
                    Ok(v) => v,
                    Err(e) => return ::std::boxed::Box::pin(::core::future::ready(Err(e))),
                };
            });

            args.push(quote! { query });
        }

        if let Some(ty) = &ep.request_type {
            extractions.push(quote_spanned! { span =>
                let body = match ::gropius::Body::<#ty>::extract(_req) {
                    Ok(v) => v,
                    Err(e) => return ::std::boxed::Box::pin(::core::future::ready(Err(e))),
                };
            });

            args.push(quote! { body });
        }

        if ep.raw_request {
            extractions.push(quote! {
                let raw_request = _req.clone();
            });

            args.push(quote! { raw_request });
        }

        let ok_branch = match &ep.response_kind {
            ResponseKind::Json(_) => quote_spanned! { span =>
                ::gropius::generated::make_json_response(&v, ::http::StatusCode::OK)
            },
            ResponseKind::Empty => quote! {
                Ok(::http::Response::builder()
                    .status(::http::StatusCode::OK)
                    .body(::bytes::Bytes::new())
                    .unwrap())
            },
            ResponseKind::Raw => quote! { Ok(v) },
        };

        quote_spanned! { span =>
            ::std::sync::Arc::new({
                let this = self.clone();
                move |_req, _path_params| {
                    #(#extractions)*

                    let this = this.clone();
                    ::std::boxed::Box::pin(async move {
                        match this.#name(#(#args),*).await {
                            Ok(v) => { let _ = v; #ok_branch },
                            Err(e) => {
                                let status = ::gropius::ApiError::status_code(&e);
                                ::gropius::generated::make_json_response(&e, status)
                            }
                        }
                    })
                }
            })
        }
    });

    let trait_doc_expr = quote_option(trait_doc.as_ref());
    let vis = &item_trait.vis;

    let trait_ident = &item_trait.ident;
    let trait_name = trait_ident.to_string();

    let mut const_name = trait_name.to_shouty_snake_case();
    if !trait_name.ends_with("Spec") {
        const_name.push_str("_SPEC");
    }

    let const_ident = syn::Ident::new(&const_name, trait_ident.span());

    if !errors.is_empty() {
        let error_tokens = errors.into_iter().map(|d| d.emit_as_item_tokens());
        return quote! {
            #item_trait
            #(#error_tokens)*
        };
    }

    let endpoints_method: syn::TraitItem = syn::parse_quote! {
        fn endpoints(&self) -> ::gropius::generated::ApiImpl
        where
            Self: ::core::clone::Clone + ::core::marker::Send + ::core::marker::Sync + 'static,
        {
            let handlers: ::std::vec::Vec<::gropius::generated::Handler> = vec![#(#handler_tokens),*];

            ::gropius::generated::ApiImpl {
                attributes: #const_ident.attr,
                handlers: #const_ident.endpoints.iter().zip(handlers).collect(),
            }
        }
    };

    item_trait.items.push(endpoints_method);

    // Generate type assertions for the extractor inner types, the request and
    // response types, and the error type, with spans pointing at the place of
    // use.
    let assertion_tokens = endpoints.iter().map(|ep| {
        let mut checks = Vec::new();

        for ty in [&ep.path_type, &ep.query_type, &ep.request_type]
            .into_iter()
            .flatten()
        {
            checks.push(quote_spanned! { ty.span() => {
                fn check<T: ::serde::de::DeserializeOwned + ::schemars::JsonSchema>() {}
                check::<#ty>();
            }});
        }

        if let ResponseKind::Json(ty) = &ep.response_kind {
            checks.push(quote_spanned! { ty.span() => {
                fn check<T: ::serde::Serialize + ::schemars::JsonSchema>() {}
                check::<#ty>();
            }});
        }

        let error_type = &ep.error_type;
        checks.push(quote_spanned! { error_type.span() => {
            fn check<T: ::gropius::ApiError + ::serde::Serialize + ::schemars::JsonSchema>() {}
            check::<#error_type>();
        }});

        quote! { #(#checks)* }
    });

    quote! {
        #item_trait

        const _: fn() = || {
            #(#assertion_tokens)*
        };

        #vis const #const_ident: ::gropius::generated::Api = ::gropius::generated::Api {
            attr: ::gropius::generated::ApiAttributes {
                tags: &[#(#tags),*],
                doc: #trait_doc_expr,
            },
            endpoints: &[#(#desc_tokens),*],
        };
    }
}

/// Parses an endpoint method. Removes the `#[endpoint(...)] annotation.
fn parse_endpoint(method: &mut TraitItemFn) -> Result<RawEndpoint, Diagnostic> {
    let span = method.sig.ident.span();
    desugar_async(&mut method.sig);

    let attr = method
        .attrs
        .extract_if(.., |a| a.path().is_ident("endpoint"))
        .next();

    let Some(attr) = attr else {
        return Err(span.error("expected #[endpoint(...)] attribute on method"));
    };

    let parsed: EndpointAttr = attr
        .parse_args()
        .map_err(|e| e.span().error(e.to_string()))?;

    let mut path_type = None;
    let mut query_type = None;
    let mut request_type = None;
    let mut raw_request = false;

    let mut inputs = method.sig.inputs.iter();
    match inputs.next() {
        Some(FnArg::Receiver(r)) if r.mutability.is_none() => (),
        _ => {
            return Err(span.error("expected `&self` as first argument"));
        }
    };

    for input in inputs {
        let FnArg::Typed(PatType { ty, .. }) = input else {
            unreachable!()
        };

        // Point at the extractor itself (`Path`/`Query`/`Body`), not the
        // binding or the leading path segment.
        let extractor_span = if let Type::Path(p) = ty.deref()
            && let Some(last) = p.path.segments.last()
        {
            last.ident.span()
        } else {
            input.span()
        };
        let out_of_order =
            extractor_span.error("arguments must be in order: Path, Query, Body, Request");

        if let Some(inner) = parse_extractor(ty, "Path") {
            if query_type.is_some() || request_type.is_some() || raw_request {
                return Err(out_of_order);
            }

            path_type = Some(inner);
        } else if let Some(inner) = parse_extractor(ty, "Query") {
            if request_type.is_some() || raw_request {
                return Err(out_of_order);
            }

            query_type = Some(inner);
        } else if let Some(inner) = parse_extractor(ty, "Body") {
            if raw_request {
                return Err(out_of_order);
            }

            request_type = Some(inner);
        } else if let Type::Path(p) = ty.deref()
            && let Some(last) = p.path.segments.last()
            && last.ident == "Request"
        {
            raw_request = true;
        } else {
            return Err(ty
                .span()
                .error("expected Path<T>, Query<T>, Body<T>, or Request"));
        }
    }

    let Some((response_type, error_type)) = parse_result_type(&method.sig.output) else {
        let return_type_span = match &method.sig.output {
            ReturnType::Type(_, ty) => ty.span(),
            ReturnType::Default => method.sig.ident.span(),
        };

        return Err(return_type_span.error("expected return type Result<R, E>"));
    };

    let response_kind = if let Type::Path(p) = &response_type
        && let Some(last) = p.path.segments.last()
    {
        match last.ident.to_string().as_str() {
            "EmptyResponse" => ResponseKind::Empty,
            "Response" => ResponseKind::Raw,
            _ => ResponseKind::Json(Box::new(response_type)),
        }
    } else {
        ResponseKind::Json(Box::new(response_type))
    };

    Ok(RawEndpoint {
        span,
        name: method.sig.ident.clone(),
        method: parsed.method,
        path: parsed.path,
        doc: extract_doc(&method.attrs),
        path_type,
        query_type,
        request_type,
        raw_request,
        response_kind,
        content_type: parsed.content_type,
        error_type,
    })
}

fn quote_option<T: quote::ToTokens>(opt: Option<&T>) -> TokenStream {
    match opt {
        Some(val) => quote! { Some(#val) },
        None => quote! { None },
    }
}

/// Strip `async` and rewrite `-> R` to `-> impl Future<Output = R> + Send`.
fn desugar_async(sig: &mut syn::Signature) {
    if sig.asyncness.take().is_none() {
        return;
    }

    let output: Type = match &sig.output {
        ReturnType::Type(_, ty) => *ty.clone(),
        ReturnType::Default => syn::parse_quote! { () },
    };

    sig.output = syn::parse_quote! {
        -> impl ::core::future::Future<Output = #output> + ::core::marker::Send
    };
}

/// If `ty` is `Wrapper<T>` where the last path segment is `name`, return `T`.
fn parse_extractor(ty: &Type, name: &str) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    let last = type_path.path.segments.last()?;
    if last.ident != name {
        return None;
    }

    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };

    let GenericArgument::Type(inner) = args.args.first()? else {
        return None;
    };

    Some(inner.clone())
}

/// Extract `(R, E)` from `-> impl Future<Output = Result<R, E>>`.
fn parse_result_type(output: &ReturnType) -> Option<(Type, Type)> {
    if let ReturnType::Type(_, ty) = output
        && let Type::ImplTrait(impl_trait) = ty.deref()
        && let TypeParamBound::Trait(trait_bound) = impl_trait.bounds.first()?
        && let last = trait_bound.path.segments.last()?
        && last.ident == "Future"
        && let PathArguments::AngleBracketed(args) = &last.arguments
        && let GenericArgument::AssocType(assoc) = args.args.first()?
        && let Type::Path(type_path) = &assoc.ty
        && let last = type_path.path.segments.last()?
        && last.ident == "Result"
        && let PathArguments::AngleBracketed(args) = &last.arguments
        && let GenericArgument::Type(r) = args.args.first()?
        && let GenericArgument::Type(e) = args.args.last()?
    {
        Some((r.clone(), e.clone()))
    } else {
        None
    }
}

fn schema_fn(ty: Option<&Type>, optional: bool, span: Span) -> TokenStream {
    match (ty, optional) {
        (Some(ty), true) => {
            quote_spanned! { span => Some(<#ty as ::schemars::JsonSchema>::json_schema) }
        }
        (Some(ty), false) => {
            quote_spanned! { span => <#ty as ::schemars::JsonSchema>::json_schema }
        }
        (None, _) => quote! { None },
    }
}

/// Collects `#[doc = "..."]` attributes into a single string.
fn extract_doc(attrs: &[syn::Attribute]) -> Option<String> {
    let mut lines = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc")
            && let syn::Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
        {
            lines.push(s.value());
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n").trim().to_owned())
    }
}
