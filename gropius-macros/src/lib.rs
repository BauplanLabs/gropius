use proc_macro::TokenStream;

mod api;
mod api_error;

#[proc_macro_attribute]
pub fn api(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_trait = syn::parse_macro_input!(item as syn::ItemTrait);

    api::expand(attr.into(), item_trait).into()
}

/// Derives the `gropius::ApiError` trait.
///
/// Each enum variant maps to an HTTP status with `#[api_error(...)]`, taking
/// a bare code like `404` (range-checked at compile time) or a `StatusCode`
/// constant; a struct takes a single one on the container. A newtype variant
/// can delegate to its inner error with `#[api_error(transparent)]`, to share
/// variants between enums.
#[proc_macro_derive(ApiError, attributes(api_error))]
pub fn derive_api_error(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    api_error::expand(input).into()
}
