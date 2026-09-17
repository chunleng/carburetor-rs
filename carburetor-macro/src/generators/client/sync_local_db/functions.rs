use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::Ident;

use crate::{
    generators::{
        client::models::AsTableMetadata,
        diesel::{
            models::{AsChangesetModel, AsFullModel, AsInsertModel, AsModelType},
            schema::AsSchemaTable,
        },
        download::models::{AsDownloadResponseModel, AsDownloadResponseTableModel},
    },
    parsers::{
        sync_group::CarburetorSyncGroup,
        table::{
            CarburetorTable,
            column::{CarburetorColumnType, ColumnScope},
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
        let insert_model_name = AsInsertModel(self.table).get_model_name();
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
                match (&x.column_type, &x.column_scope) {
                    // Similar to to AsTableMetadata, we are only interested on non-metadata
                    // columns (data columns) that are synced to the backend eventually
                    // (non-client-only) here.
                    //
                    // Non-data columns (id, last_synced_at, etc.) are used to ensure syncing work,
                    // and client-only data will never need to be synced to the server.
                    (CarburetorColumnType::Data, ColumnScope::Both | ColumnScope::ModOnBackendOnly) => {
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
                                    let mut existing_metadata: ClientSyncMetadata<#table_metadata_model_name>;
                                    existing_metadata = from_value(existing_item.#column_sync_metadata_column_name).unwrap_or_default();

                                    #(#check_dirty_columns)*

                                    if !update_item.unknown_data.is_empty() {
                                        existing_metadata.stage_unknown_data(&update_item.unknown_data);
                                        update_model.#column_sync_metadata_column_name =
                                            Some(carburetor::serde_json::Value::from(existing_metadata));
                                    }

                                    diesel::update(table.find(existing_item.#id_column_name))
                                        .set(update_model)
                                        .execute(conn)?;
                                }
                            }
                            None => {
                                let mut insert_model = #insert_model_name::from(update_item.clone());
                                if !update_item.unknown_data.is_empty() {
                                    let mut metadata: ClientSyncMetadata<#table_metadata_model_name> =
                                        ClientSyncMetadata::default();
                                    metadata.stage_unknown_data(&update_item.unknown_data);
                                    insert_model.#column_sync_metadata_column_name =
                                        carburetor::serde_json::Value::from(metadata);
                                }
                                diesel::insert_into(table)
                                    .values(insert_model)
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

struct AsBackfillTableFunction<'a> {
    table: &'a CarburetorTable,
}

impl<'a> AsBackfillTableFunction<'a> {
    fn get_function_name(&self) -> Ident {
        Ident::new(
            &format!("backfill_{}", self.table.ident),
            self.table.ident.span(),
        )
    }
}

impl<'a> ToTokens for AsBackfillTableFunction<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let function_name = self.get_function_name();
        let table_name = AsSchemaTable(self.table).get_table_name();
        let table_name_str = table_name.to_string();
        let changeset_model_name = AsChangesetModel(self.table).get_model_name();
        let table_metadata_model_name = AsTableMetadata(self.table).get_struct_name();
        let id_column_name = &self.table.sync_metadata_columns.id.ident;
        let id_type = AsModelType(&self.table.sync_metadata_columns.id.diesel_type);
        let column_sync_metadata_column_name = &self
            .table
            .sync_metadata_columns
            .client_column_sync_metadata
            .ident;
        let metadata_column_str = column_sync_metadata_column_name.to_string();

        // The changeset starts with only the id set; applied columns are filled in by the
        // backfill callback below. `None` fields are skipped by `AsChangeset`.
        let changeset_fields = self.table.columns.iter().map(|x| {
            let column_name = &x.ident;
            match x.column_type {
                CarburetorColumnType::Id => quote!(#column_name: row_id.clone()),
                _ => quote!(#column_name: None),
            }
        });

        // Only data columns synced from the backend can have staged values (same filter as
        // check_dirty_columns).
        let apply_columns =
            self.table
                .columns
                .iter()
                .filter_map(|x| match (&x.column_type, &x.column_scope) {
                    (
                        CarburetorColumnType::Data,
                        ColumnScope::Both | ColumnScope::ModOnBackendOnly,
                    ) => {
                        let column_name = &x.ident;
                        let column_name_str = x.ident.to_string();
                        let ty = AsModelType(&x.diesel_type);
                        let assignment = match x.column_scope {
                            ColumnScope::ModOnBackendOnly => {
                                quote!(update_model.#column_name = Some(Some(converted)))
                            }
                            _ => quote!(update_model.#column_name = Some(converted)),
                        };
                        Some(quote! {
                            #column_name_str => {
                                match carburetor::serde_json::from_value::<#ty>(value.clone()) {
                                    Ok(converted) => {
                                        #assignment;
                                        metadata.unknown_data.remove(&column);
                                        Ok(true)
                                    }
                                    Err(e) => Err(e),
                                }
                            }
                        })
                    }
                    _ => None,
                });

        tokens.extend(quote! {
            fn #function_name(
                conn: &mut diesel::SqliteConnection,
            ) -> carburetor::error::Result<Option<carburetor::error::ResetTableEntry>> {
                use diesel::prelude::*;
                use carburetor::helpers::client_sync_metadata::ClientSyncMetadata;

                let rows = #table_name::table
                    .select((
                        #table_name::#id_column_name,
                        #table_name::#column_sync_metadata_column_name,
                    ))
                    .filter(diesel::dsl::sql::<diesel::sql_types::Bool>(&format!(
                        "json_extract({}, '$.\".unknown_data\"') IS NOT NULL",
                        #metadata_column_str
                    )))
                    .load::<(#id_type, carburetor::serde_json::Value)>(conn)?;

                for (row_id, metadata_value) in rows {
                    let mut metadata: ClientSyncMetadata<#table_metadata_model_name> =
                        carburetor::serde_json::from_value(metadata_value).unwrap_or_default();
                    let mut update_model = #changeset_model_name {
                        #(#changeset_fields,)*
                    };

                    let applied = metadata
                        .unknown_data
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .try_fold(false, |applied, column| {
                            let value = &metadata.unknown_data[&column];
                            match column.as_str() {
                                #(#apply_columns)*
                                _ => Ok(applied),
                            }
                        });
                    match applied {
                        Ok(applied) => {
                            if applied {
                                update_model.#column_sync_metadata_column_name =
                                    Some(carburetor::serde_json::Value::from(metadata));
                                diesel::update(#table_name::table.find(&row_id))
                                    .set(update_model)
                                    .execute(conn)?;
                            }
                        }
                        Err(e) => {
                            // Reset the table because staged data is corrupted
                            conn.immediate_transaction(
                                |conn| -> carburetor::error::Result<()> {
                                    carburetor::helpers::carburetor_offset::delete_offset(
                                        conn,
                                        #table_name_str,
                                    )?;
                                    carburetor::helpers::client_sync_metadata::clear_table_unknown_data(
                                        conn,
                                        #table_name_str,
                                        #metadata_column_str,
                                    )?;
                                    Ok(())
                                },
                            )?;
                            return Ok(Some(carburetor::error::ResetTableEntry {
                                table_name: #table_name_str.to_string(),
                                source: e.into(),
                            }));
                        }
                    }
                }
                Ok(None)
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

pub(crate) fn generate_apply_backfill_function(
    tokens: &mut TokenStream,
    sync_group: &CarburetorSyncGroup,
) {
    let function_name = Ident::new("apply_backfill", sync_group.name.span());
    let backfill_table_functions_decl = sync_group
        .table_configs
        .iter()
        .map(|x| AsBackfillTableFunction {
            table: &x.reference_table,
        })
        .collect::<Vec<_>>();
    let call_backfill_table_function = backfill_table_functions_decl
        .iter()
        .map(|x| {
            let call_name = x.get_function_name();
            quote! {
                if let Some(entry) = #call_name(&mut conn)? {
                    resetted_tables.push(entry);
                }
            }
        })
        .collect::<Vec<_>>();

    tokens.extend(quote! {
        /// Call after a migration that added columns which previously arrived as staged unknown
        /// data (see `ClientSyncMetadata::unknown_data`). Safe to call anytime: no-op when nothing
        /// is staged, and staged entries whose columns still don't exist locally are simply kept
        /// for a later attempt.
        ///
        /// On error (`Error::ResetTable`), the named tables were reset: their sync offsets were
        /// dropped and staged data cleared, so the next regular download re-fetches them from
        /// scratch. No data is lost; just keep syncing.
        pub fn #function_name() -> carburetor::error::Result<()> {
            #(#backfill_table_functions_decl)*
            let mut conn = carburetor::helpers::get_connection()?;
            let mut resetted_tables: Vec<carburetor::error::ResetTableEntry> = Vec::new();
            #(#call_backfill_table_function)*
            if !resetted_tables.is_empty() {
                return Err(carburetor::error::Error::ResetTable {
                    errors: resetted_tables,
                });
            }
            Ok(())
        }
    });
}
