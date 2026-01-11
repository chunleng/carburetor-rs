#[cfg(feature = "client")]
pub(crate) mod client;
pub(crate) mod diesel;
pub(crate) mod download;
pub(crate) mod upload;

use proc_macro2::TokenStream;
use quote::quote;

use crate::generators::{
    diesel::{models::generate_diesel_model, schema::generate_diesel_table_schema},
    download::models::generate_download_sync_group_models,
    upload::{
        functions::generate_upload_sync_group_functions, models::generate_upload_sync_group_models,
    },
};

use super::parsers::CarburetorSyncConfig;

pub(crate) fn generate_carburetor_sync_config(
    tokens: &mut TokenStream,
    sync_config: CarburetorSyncConfig,
) {
    #[cfg(feature = "backend")]
    let mut tokens = tokens;
    #[cfg(feature = "backend")]
    sync_config.tables.iter().for_each(|x| {
        generate_diesel_table_schema(&mut tokens, &x);
        generate_diesel_model(&mut tokens, &x);
    });

    sync_config.sync_groups.iter().for_each(|x| {
        let mut mod_tokens = TokenStream::new();
        generate_download_sync_group_models(&mut mod_tokens, &x);
        generate_upload_sync_group_models(&mut mod_tokens, x);
        generate_upload_sync_group_functions(&mut mod_tokens, x);
        crate::generators::download::functions::generate_download_sync_group_functions(
            &mut mod_tokens,
            &x,
        );

        #[cfg(feature = "client")]
        {
            use crate::generators::client::{
                local_operations::{
                    functions::generate_local_operation_functions,
                    models::generate_local_operation_models,
                },
                models::generate_client_models,
                sync_local_db::functions::generate_store_download_response_function,
            };

            x.table_configs.iter().for_each(|config| {
                generate_diesel_table_schema(&mut mod_tokens, &config.reference_table);
                generate_diesel_model(&mut mod_tokens, &config.reference_table);
            });

            generate_client_models(&mut mod_tokens, &x);
            generate_store_download_response_function(&mut mod_tokens, &x);

            generate_local_operation_functions(&mut mod_tokens, &x);
            generate_local_operation_models(&mut mod_tokens, &x);
        }

        let mod_name = &x.name;
        tokens.extend(quote! {
            pub mod #mod_name {
                #mod_tokens
            }
        });
    })
}
