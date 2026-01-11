use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::Ident;

use crate::{
    generators::{
        client::models::AsTableMetadata,
        diesel::{
            models::{AsChangesetModel, AsFullModel},
            schema::AsSchemaTable,
        },
        download::models::{AsDownloadResponseModel, AsDownloadResponseTableModel},
    },
    parsers::{
        sync_group::CarburetorSyncGroup,
        table::{
            CarburetorTable,
            column::{CarburetorColumnType, ClientOnlyConfig},
        },
    },
};

struct AsSyncTableToLocalDbFunction<'a> {
    sync_group: &'a CarburetorSyncGroup,
    table: &'a CarburetorTable,
}

impl<'a> AsSyncTableToLocalDbFunction<'a> {
    fn get_function_name(&self) -> Ident {
        Ident::new(
            &format!("sync_{}", self.table.ident),
            self.table.ident.span(),
        )
    }
}

impl<'a> ToTokens for AsSyncTableToLocalDbFunction<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let function_name = self.get_function_name();
        let download_response_table_type =
            AsDownloadResponseTableModel(self.sync_group, self.table).get_type();
        let table_name = AsSchemaTable(self.table).get_table_name();
        let full_model_name = AsFullModel(self.table).get_model_name();
        let changeset_model_name = AsChangesetModel(self.table).get_model_name();
        let id_column_name = &self.table.sync_metadata_columns.id.ident;
        let last_synced_at_column_name = &self.table.sync_metadata_columns.last_synced_at.ident;
        let column_sync_metadata_column_name = &self
            .table
            .sync_metadata_columns
            .client_column_sync_metadata
            .ident;

        let table_metadata_model_name = AsTableMetadata(self.table).get_struct_name();

        let check_dirty_columns = {
            let columns = self.table.columns.clone();
            columns.into_iter().map(|x| {
                match (&x.column_type, &x.client_only_config) {
                    // Similar to to AsTableMetadata, we are only interested on non-metadata
                    // columns (data columns) that are synced to the backend eventually
                    // (non-client-only) here.
                    //
                    // Non-data columns (id, last_synced_at, etc.) are used to ensure syncing work,
                    // and client-only data will never need to be synced to the server.
                    (CarburetorColumnType::Data, ClientOnlyConfig::Disabled) => {
                        let column_name = &x.ident;
                        quote! {
                            if existing_metadata.data.as_ref().and_then(|x| { x.#column_name.as_ref() }).and_then(|x| { x.dirty_at.to_owned() }).is_some() ||
                                existing_metadata.data.as_ref().and_then(|x| { x.#column_name.as_ref() }).and_then(|x| { x.column_last_synced_at }).is_some_and(|x| {
                                    if let Some(last_synced_at) = existing_item.#last_synced_at_column_name {
                                        x > last_synced_at
                                    } else {
                                        // treat it as dirty if there's no local last_synced_at
                                        // time.
                                        true
                                    }
                                 }) {
                                update_model.#column_name = None;
                            }
                        }
                    }
                    _ => { quote!{}},
                }
            }).collect::<Vec<_>>()
        };

        tokens.extend(quote! {
            fn #function_name(
                conn: &mut diesel::SqliteConnection,
                data: Vec<carburetor::models::DownloadTableResponseData<#download_response_table_type>>,
            ) -> carburetor::error::Result<()> {
                use diesel::prelude::*;
                use carburetor::helpers::client_sync_metadata::ClientSyncMetadata;
                use carburetor::serde_json::from_value;

                data
                .into_iter()
                .map(|data_item| -> carburetor::error::Result<()> {
                    let carburetor::models::DownloadTableResponseData::Update(update_item) = data_item;
                    let table = #table_name::table;
                    conn.immediate_transaction(|conn| {
                        let maybe_existing_item = table
                            .select(#full_model_name::as_select())
                            .find(&update_item.#id_column_name)
                            .first(conn)
                            .optional()?;
                        match maybe_existing_item {
                            Some(existing_item) => {
                                let mut update_model = #changeset_model_name::from(update_item.clone());
                                if update_model
                                    .#last_synced_at_column_name
                                    .is_some_and(|x| x > existing_item.#last_synced_at_column_name)
                                {
                                    let existing_metadata: ClientSyncMetadata<#table_metadata_model_name>;
                                    existing_metadata = from_value(existing_item.#column_sync_metadata_column_name).unwrap_or_default();

                                    #(#check_dirty_columns)*

                                    diesel::update(table.find(existing_item.#id_column_name))
                                        .set(update_model)
                                        .execute(conn)?;
                                }
                            }
                            None => {
                                diesel::insert_into(table)
                                    .values(#full_model_name::from(update_item.clone()))
                                    .execute(conn)?;
                            }
                        }
                        Ok(())
                    })
                    .map_err(|e: diesel::result::Error| carburetor::error::Error::Unhandled {
                        message: "Diesel error has occurred".to_string(),
                        source: e.into(),
                    })?;
                    Ok(())
                })
                .collect::<carburetor::error::Result<Vec<_>>>()?;

                Ok(())
            }
        });
    }
}

pub(crate) fn generate_store_download_response_function(
    tokens: &mut TokenStream,
    sync_group: &CarburetorSyncGroup,
) {
    let function_name = Ident::new("store_download_response", sync_group.name.span());
    let download_response_model = AsDownloadResponseModel(sync_group);
    let download_response_model_name = download_response_model.get_model_name();
    let sync_table_functions_decl = sync_group
        .table_configs
        .iter()
        .map(|x| AsSyncTableToLocalDbFunction {
            sync_group,
            table: &x.reference_table,
        })
        .collect::<Vec<_>>();
    let call_sync_table_function = sync_table_functions_decl
        .iter()
        .map(|x| {
            let call_name = x.get_function_name();
            let field = download_response_model
                .get_response_field_by_table(x.table)
                .get_field_name();
            let table_name = AsSchemaTable(x.table).get_table_name().to_string();
            quote! {
                #call_name(&mut conn, download_response.#field.data)?;
                carburetor::helpers::carburetor_offset::upsert_offset(
                    &mut conn,
                    #table_name,
                    download_response.#field.cutoff_at,
                )?;
            }
        })
        .collect::<Vec<_>>();

    tokens.extend(quote! {
        pub fn #function_name(
            download_response: #download_response_model_name,
        ) -> carburetor::error::Result<()> {
            #(#sync_table_functions_decl)*
            let mut conn = carburetor::helpers::get_connection()?;
            #(#call_sync_table_function)*
            Ok(())
        }
    });
}
