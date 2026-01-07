pub(crate) mod diesel;
pub(crate) mod download;

use proc_macro2::TokenStream;

use crate::generators::{
    diesel::{models::generate_diesel_model, schema::generate_diesel_table_schema},
    download::{
        functions::generate_download_sync_group_functions,
        models::generate_download_sync_group_models,
    },
};

use super::parsers::CarburetorSyncConfig;

pub(crate) fn generate_carburetor_sync_config(
    mut tokens: &mut TokenStream,
    sync_config: CarburetorSyncConfig,
) {
    sync_config.tables.iter().for_each(|x| {
        generate_diesel_table_schema(&mut tokens, &x.borrow());
        generate_diesel_model(&mut tokens, &x.borrow());
    });
    sync_config.sync_groups.iter().for_each(|x| {
        generate_download_sync_group_models(&mut tokens, &x);
        generate_download_sync_group_functions(&mut tokens, &x);
    })
}
