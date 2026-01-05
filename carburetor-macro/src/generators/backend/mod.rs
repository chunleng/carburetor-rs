use proc_macro::TokenStream;
use syn::Result;

use crate::{CarburetorArgs, TableDetail, generators::backend::diesel::generate_diesel};

pub(crate) mod diesel;

pub(crate) fn generate_backend(args: &CarburetorArgs, table: &TableDetail) -> Result<TokenStream> {
    let diesel = generate_diesel(args, table)?;

    Ok(diesel)
}
