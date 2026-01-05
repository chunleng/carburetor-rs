use proc_macro::TokenStream;
use quote::quote;
use syn::Result;

use crate::{
    CarburetorArgs, TableDetail,
    generators::backend::{diesel::generate_diesel, sync::generate_sync_functions},
};

pub(crate) mod diesel;
pub(crate) mod sync;

pub(crate) fn generate_backend(args: &CarburetorArgs, table: &TableDetail) -> Result<TokenStream> {
    let diesel_output = generate_diesel(args, table)?;
    let sync_functions = generate_sync_functions(args, table)?;

    Ok(quote! {
        #diesel_output
        #sync_functions
    }
    .into())
}
