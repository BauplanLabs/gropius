use proc_macro::TokenStream;

mod api;

#[proc_macro_attribute]
pub fn api(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_trait = syn::parse_macro_input!(item as syn::ItemTrait);

    api::expand(attr.into(), item_trait).into()
}
