use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::Ident;

use crate::{
    generators::{
        client::{
            local_operations::models::{AsLocalInsertModel, AsLocalUpdateModel},
            models::AsTableMetadata,
        },
        diesel::{
            models::{AsChangesetModel, AsFullModel, AsModelType},
            schema::AsSchemaTable,
        },
    },
    parsers::{
        sync_group::{CarburetorSyncGroup, SyncGroupTableConfig},
        table::column::CarburetorColumnType,
    },
};

struct AsLocalInsertFunction<'a>(&'a SyncGroupTableConfig);

impl<'a> ToTokens for AsLocalInsertFunction<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let function_name = self.get_function_name();
        let insert_model_name = AsLocalInsertModel(self.0).get_model_name();
        let full_model_name = AsFullModel(&self.0.reference_table).get_model_name();
        let table_name = AsSchemaTable(&self.0.reference_table).get_table_name();
        tokens.extend(quote!(
            pub fn #function_name(insert_value: #insert_model_name) -> carburetor::error::Result<#full_model_name> {
                use diesel::{RunQueryDsl, Connection};
                Ok(
                    diesel::insert_into(#table_name::table)
                        .values(#full_model_name::from(insert_value))
                        .get_result(&mut carburetor::helpers::get_connection()?)
                        .map_err(|e| carburetor::error::Error::Unhandled {
                            message: "record insertion failed".to_string(),
                            source: e.into(),
                    })?
                )
            }
        ));
    }
}

impl<'a> AsLocalInsertFunction<'a> {
    fn get_function_name(&self) -> Ident {
        format_ident!("insert_{}", self.0.reference_table.ident)
    }
}

struct AsLocalUpdateFunction<'a>(&'a SyncGroupTableConfig);

impl<'a> ToTokens for AsLocalUpdateFunction<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let function_name = self.get_function_name();
        let update_model_name = AsLocalUpdateModel(self.0).get_model_name();
        let changeset_model_name = AsChangesetModel(&self.0.reference_table).get_model_name();
        let full_model_name = AsFullModel(&self.0.reference_table).get_model_name();
        let table_name = AsSchemaTable(&self.0.reference_table).get_table_name();
        let id_column_name = &self.0.reference_table.sync_metadata_columns.id.ident;
        let dirty_column_name = &self
            .0
            .reference_table
            .sync_metadata_columns
            .dirty_flag
            .ident;
        let client_metadata_model_name = AsTableMetadata(&self.0.reference_table).get_struct_name();
        let client_metadata_column_name = &self
            .0
            .reference_table
            .sync_metadata_columns
            .client_column_sync_metadata
            .ident;
        let check_data_column_change = &self
            .0
            .reference_table
            .columns
            .iter()
            .filter_map(|x| {
                let column_name = &x.ident;
                match x.column_type {
                    CarburetorColumnType::Data => Some(quote! {
                        if changeset.#column_name.is_some() {
                            new_metadata
                                .data
                                .get_or_insert_default()
                                .#column_name
                                .get_or_insert_default()
                                .dirty_at = Some(carburetor::helpers::get_utc_now());
                        }
                    }),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        tokens.extend(quote!(
            pub fn #function_name(update_value: #update_model_name) -> carburetor::error::Result<#full_model_name> {
                use diesel::{RunQueryDsl, Connection, QueryDsl, SelectableHelper};
                let mut changeset = #changeset_model_name::from(update_value);
                let changeset_id = changeset.#id_column_name.clone();
                let mut conn = carburetor::helpers::get_connection()?;
                Ok(
                    conn.immediate_transaction(|conn| -> Result<#full_model_name, diesel::result::Error> {
                        let existing_item = #table_name::table
                            .select(#full_model_name::as_select())
                            .find(&changeset.#id_column_name)
                            .first(conn)?;

                        if existing_item.#dirty_column_name.is_none() {
                            changeset.#dirty_column_name = Some(
                                Some(carburetor::helpers::client_sync_metadata::DirtyFlag::Update.to_string())
                            );
                        }

                        let mut new_metadata: carburetor::helpers::client_sync_metadata::ClientSyncMetadata<#client_metadata_model_name> = carburetor::serde_json::from_value(existing_item.#client_metadata_column_name).unwrap_or_default();
                        #(#check_data_column_change)*
                        changeset.#client_metadata_column_name = Some(new_metadata.into());
                        Ok(
                            diesel::update(#table_name::table.find(changeset_id))
                                .set(changeset)
                                .get_result(conn)?
                        )
                    })
                    .map_err(|e| carburetor::error::Error::Unhandled {
                        message: "error has occurred in diesel while attempting to update record".to_string(),
                        source: e.into(),
                    })?
                )
            }
        ));
    }
}

impl<'a> AsLocalUpdateFunction<'a> {
    fn get_function_name(&self) -> Ident {
        format_ident!("update_{}", self.0.reference_table.ident)
    }
}

struct AsLocalDeleteFunction<'a>(&'a SyncGroupTableConfig);

impl<'a> ToTokens for AsLocalDeleteFunction<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let function_name = self.get_function_name();
        let id_type = AsModelType(&self.0.reference_table.sync_metadata_columns.id.diesel_type);
        let changeset_model_name = AsChangesetModel(&self.0.reference_table).get_model_name();
        let changeset_fields = self.0.reference_table.columns.iter().map(|x| {
            let field_name = &x.ident;
            match &x.column_type {
                CarburetorColumnType::Id => quote!(#field_name: delete_id),
                CarburetorColumnType::IsDeleted => quote!(#field_name: Some(true)),
                _ => quote!(#field_name: None),
            }
        });

        let full_model_name = AsFullModel(&self.0.reference_table).get_model_name();
        let table_name = AsSchemaTable(&self.0.reference_table).get_table_name();

        let id_column_name = &self.0.reference_table.sync_metadata_columns.id.ident;
        let delete_column_name = &self
            .0
            .reference_table
            .sync_metadata_columns
            .is_deleted
            .ident;
        let dirty_column_name = &self
            .0
            .reference_table
            .sync_metadata_columns
            .dirty_flag
            .ident;
        let client_metadata_model_name = AsTableMetadata(&self.0.reference_table).get_struct_name();
        let client_metadata_column_name = &self
            .0
            .reference_table
            .sync_metadata_columns
            .client_column_sync_metadata
            .ident;
        tokens.extend(quote!(
            pub fn #function_name(delete_id: #id_type) -> carburetor::error::Result<#full_model_name> {
                use diesel::{RunQueryDsl, Connection, QueryDsl, SelectableHelper};
                let mut changeset = #changeset_model_name {
                    #(#changeset_fields,)*
                };
                let changeset_id = changeset.#id_column_name.clone();
                let mut conn = carburetor::helpers::get_connection()?;
                Ok(
                    conn.immediate_transaction(|conn| -> Result<#full_model_name, diesel::result::Error> {
                        let existing_item = #table_name::table
                            .select(#full_model_name::as_select())
                            .find(&changeset.#id_column_name)
                            .first(conn)?;

                        if existing_item.#dirty_column_name.is_none() {
                            changeset.#dirty_column_name = Some(
                                Some(carburetor::helpers::client_sync_metadata::DirtyFlag::Update.to_string())
                            );
                        }

                        let mut new_metadata: carburetor::helpers::client_sync_metadata::ClientSyncMetadata<#client_metadata_model_name> = carburetor::serde_json::from_value(existing_item.#client_metadata_column_name).unwrap_or_default();
                        new_metadata
                            .data
                            .get_or_insert_default()
                            .#delete_column_name
                            .get_or_insert_default()
                            .dirty_at = Some(carburetor::helpers::get_utc_now());
                        changeset.#client_metadata_column_name = Some(new_metadata.into());

                        Ok(
                            diesel::update(#table_name::table.find(changeset_id))
                                .set(changeset)
                                .get_result(conn)?
                        )
                    })
                    .map_err(|e| carburetor::error::Error::Unhandled {
                        message: "error has occurred in diesel while attempting to delete record".to_string(),
                        source: e.into(),
                    })?
                )
            }
        ));
    }
}

impl<'a> AsLocalDeleteFunction<'a> {
    fn get_function_name(&self) -> Ident {
        format_ident!("delete_{}", self.0.reference_table.ident)
    }
}

struct AsActiveTableFunction<'a>(&'a SyncGroupTableConfig);

impl<'a> ToTokens for AsActiveTableFunction<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let function_name = self.get_function_name();
        let table_name = AsSchemaTable(&self.0.reference_table).get_table_name();
        let is_deleted_column_name = &self
            .0
            .reference_table
            .sync_metadata_columns
            .is_deleted
            .ident;
        tokens.extend(quote!(
            pub fn #function_name() -> #table_name::BoxedQuery<'static, diesel::sqlite::Sqlite> {
                use diesel::{QueryDsl, ExpressionMethods};
                #table_name::table
                    .filter(#table_name::#is_deleted_column_name.eq(false))
                    .into_boxed()
            }
        ));
    }
}

impl<'a> AsActiveTableFunction<'a> {
    fn get_function_name(&self) -> Ident {
        format_ident!("active_{}", self.0.reference_table.plural_ident)
    }
}

pub fn generate_local_operation_functions(
    tokens: &mut TokenStream,
    sync_group: &CarburetorSyncGroup,
) {
    sync_group.table_configs.iter().for_each(|x| {
        tokens.extend(AsLocalInsertFunction(x).to_token_stream());
        tokens.extend(AsLocalUpdateFunction(x).to_token_stream());
        tokens.extend(AsLocalDeleteFunction(x).to_token_stream());
        tokens.extend(AsActiveTableFunction(x).to_token_stream());
    })
}

