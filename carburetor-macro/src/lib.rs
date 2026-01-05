mod generators;
mod parsers;

use proc_macro::TokenStream;
use syn::parse_macro_input;

use crate::{
    generators::generate_all,
    parsers::{arg::CarburetorArgs, input::TableDetail},
};

#[proc_macro_attribute]
pub fn carburetor(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as CarburetorArgs);
    let table = parse_macro_input!(item as TableDetail);
    generate_all(args, table).unwrap_or_else(|e| e.to_compile_error().into())
}
