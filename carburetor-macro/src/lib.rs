mod generators;
mod parsers;

use proc_macro::TokenStream;
use syn::parse_macro_input;

use crate::{
    generators::generate_all,
    parsers::{CarburetorTable, attr::CarburetorAttr, input::CarburetorItem},
};

#[proc_macro_attribute]
pub fn carburetor(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_parsed = parse_macro_input!(attr as CarburetorAttr);
    let item_parsed = parse_macro_input!(item as CarburetorItem);
    let table = match CarburetorTable::new(attr_parsed, item_parsed) {
        Ok(t) => t,
        Err(e) => return e.to_compile_error().into(),
    };
    generate_all(table).unwrap_or_else(|e| e.to_compile_error().into())
}
