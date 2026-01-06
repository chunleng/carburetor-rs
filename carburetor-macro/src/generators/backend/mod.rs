use proc_macro::TokenStream;
use quote::quote;
use syn::Result;

use crate::{
    CarburetorTable,
    generators::backend::{diesel::generate_diesel, sync::generate_sync_functions},
};

pub(crate) mod diesel;
pub(crate) mod sync;

pub(crate) fn generate_backend(table: &CarburetorTable) -> Result<TokenStream> {
    let diesel_output = generate_diesel(table)?;
    let sync_functions = generate_sync_functions(table)?;

    Ok(quote! {
        #diesel_output
        #sync_functions
    }
    .into())
}
