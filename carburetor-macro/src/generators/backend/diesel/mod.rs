use proc_macro2::TokenStream;
use quote::quote;
use syn::Result;

use crate::{
    CarburetorTable,
    generators::backend::diesel::{model::generate_diesel_models, table::generate_diesel_table},
};

pub(crate) mod model;
pub(crate) mod table;

pub(crate) fn generate_diesel(table: &CarburetorTable) -> Result<TokenStream> {
    let models = generate_diesel_models(&table);
    let table = generate_diesel_table(&table)?;

    Ok(quote! {
        #table
        #models
    }
    .into())
}
