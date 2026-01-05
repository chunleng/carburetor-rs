use proc_macro2::TokenStream;
use quote::quote;
use syn::Result;

use crate::{
    CarburetorArgs, TableDetail,
    generators::backend::diesel::{model::generate_diesel_models, table::generate_diesel_table},
};

pub(crate) mod model;
pub(crate) mod table;

pub(crate) fn generate_diesel(args: &CarburetorArgs, table: &TableDetail) -> Result<TokenStream> {
    let models = generate_diesel_models(&args, &table);
    let table = generate_diesel_table(&args, &table)?;

    Ok(quote! {
        #table
        #models
    }
    .into())
}
