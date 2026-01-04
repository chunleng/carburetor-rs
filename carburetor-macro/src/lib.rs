mod generators;
mod parsers;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Result, parse_macro_input};

use crate::{
    generators::diesel::{model::generate_diesel_models, table::generate_diesel_table},
    parsers::{arg::CarburetorArgs, input::TableDetail},
};

#[proc_macro_attribute]
pub fn carburetor(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as CarburetorArgs);
    let table = parse_macro_input!(item as TableDetail);
    run_macro(args, table).unwrap_or_else(|e| e.to_compile_error().into())
}

fn run_macro(args: CarburetorArgs, table: TableDetail) -> Result<TokenStream> {
    let models = generate_diesel_models(&table, &args);
    let table = generate_diesel_table(&table, &args)?;

    Ok(quote! {
        #table
        #models
    }
    .into())
}
