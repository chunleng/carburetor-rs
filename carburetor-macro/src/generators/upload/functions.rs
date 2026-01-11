use proc_macro2::TokenStream;

use crate::parsers::sync_group::CarburetorSyncGroup;

#[cfg(feature = "client")]
mod client {
    use proc_macro2::TokenStream;
    use quote::{ToTokens, format_ident, quote};
    use syn::Ident;

    use crate::{
        generators::{
            client::models::AsTableMetadata,
            diesel::{models::AsFullModel, schema::AsSchemaTable},
            upload::models::{
                AsUploadRequest, AsUploadRequestTable, AsUploadResponseModel,
                client::AsFromFullToTable,
            },
        },
        parsers::{
            sync_group::{CarburetorSyncGroup, SyncGroupTableConfig},
            table::column::{BackendOnlyConfig, CarburetorColumnType, ClientOnlyConfig},
        },
    };

    struct AsRetrieveTableUploadFunction<'a>(&'a SyncGroupTableConfig);

    impl<'a> AsRetrieveTableUploadFunction<'a> {
        fn get_function_name(&self) -> Ident {
            format_ident!("retrieve_{}_upload_data", self.0.reference_table.ident)
        }
    }

    impl<'a> ToTokens for AsRetrieveTableUploadFunction<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let function_name = self.get_function_name();
            let upload_table_model_name = AsUploadRequestTable(self.0).get_model_name();
            let table_name = AsSchemaTable(&self.0.reference_table).get_table_name();
            let full_model_name = AsFullModel(&self.0.reference_table).get_model_name();
            let fail_execution_message =
                format!("fail to retrieve dirty {} data from database", table_name);
            let dirty_flag_col_name = &self
                .0
                .reference_table
                .sync_metadata_columns
                .dirty_flag
                .ident;
            let client_metadata_col_name = &self
                .0
                .reference_table
                .sync_metadata_columns
                .client_column_sync_metadata
                .ident;
            let max_sqlite_date = "9999-12-31T23:59:59Z";
            let dirty_insert_cutoff_filter = format!(
                r#"coalesce(json_extract({}.{}, '$.".insert_time"'), '{}')"#,
                table_name, client_metadata_col_name, max_sqlite_date
            );
            let dirty_update_filters = self
                .0
                .reference_table
                .columns
                .iter()
                .filter(|x| {
                    x.client_only_config == ClientOnlyConfig::Disabled
                        && x.mod_on_backend_only_config == BackendOnlyConfig::Disabled
                })
                .fold(
                    quote!(diesel::dsl::sql::<diesel::sql_types::Bool>("false")),
                    |acc, x| {
                        let dirty_update_cutoff_filter = format!(
                            "coalesce(json_extract({}.{}, '$.{}.dirty_at'), '{}')",
                            table_name, client_metadata_col_name, x.ident, max_sqlite_date
                        );
                        quote! {
                            diesel::dsl::sql::<diesel::sql_types::Text>(
                                #dirty_update_cutoff_filter
                            )
                            .le(&cutoff_time_rfc3339)
                            .or(#acc),
                        }
                    },
                );
            let into_upload_request_function_name = AsFromFullToTable(self.0).get_function_name();

            tokens.extend(quote! {
                fn #function_name(
                    cutoff_time: carburetor::chrono::DateTimeUtc,
                ) -> carburetor::error::Result<Vec<#upload_table_model_name>> {
                    let mut connection = carburetor::helpers::get_connection()?;
                    let cutoff_time_rfc3339 = cutoff_time.to_rfc3339();
                    Ok(#table_name::table
                        .select(#full_model_name::as_select())
                        .filter(
                            #table_name::dsl::#dirty_flag_col_name
                            .eq(carburetor::helpers::client_sync_metadata::DirtyFlag::Insert
                                .to_string())
                            .and(
                                diesel::dsl::sql::<diesel::sql_types::Text>(
                                    #dirty_insert_cutoff_filter
                                )
                                .le(&cutoff_time_rfc3339),
                            )
                            .or(#table_name::dsl::#dirty_flag_col_name
                                .eq(carburetor::helpers::client_sync_metadata::DirtyFlag::Update
                                    .to_string())
                                .and(
                                    #dirty_update_filters
                                )),
                        )
                        .load::<#full_model_name>(&mut connection)
                        .map_err(|e| carburetor::error::Error::Unhandled {
                            message: #fail_execution_message.to_string(),
                            source: e.into(),
                        })?
                    .into_iter()
                        .filter_map(|x| x.#into_upload_request_function_name(cutoff_time))
                        .collect::<Vec<_>>())
                }
            });
        }
    }

    pub struct AsRetrieveUploadFunction<'a>(pub &'a CarburetorSyncGroup);

    impl<'a> ToTokens for AsRetrieveUploadFunction<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let table_upload_functions = self
                .0
                .table_configs
                .iter()
                .map(|x| AsRetrieveTableUploadFunction(x))
                .collect::<Vec<_>>();

            let upload_request_model_name = AsUploadRequest(self.0).get_model_name();
            let upload_request_fields = self
                .0
                .table_configs
                .iter()
                .map(|x| {
                    let field = &x.reference_table.ident;
                    let function_name = AsRetrieveTableUploadFunction(x).get_function_name();
                    quote!(#field: #function_name(cutoff_time)?)
                })
                .collect::<Vec<_>>();

            tokens.extend(quote! {
                pub fn retrieve_upload_request()
                -> carburetor::error::Result<(carburetor::chrono::DateTimeUtc, #upload_request_model_name)> {
                    use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};

                    #(#table_upload_functions)*

                    let cutoff_time = carburetor::helpers::get_utc_now();
                    Ok((
                        cutoff_time,
                        #upload_request_model_name {
                            #(#upload_request_fields,)*
                        },
                    ))
                }
            });
        }
    }

    struct AsProcessTableUploadResponseFunction<'a>(&'a SyncGroupTableConfig);

    impl<'a> AsProcessTableUploadResponseFunction<'a> {
        fn get_function_name(&self) -> Ident {
            format_ident!("process_{}_upload_response", self.0.reference_table.ident)
        }
    }

    impl<'a> ToTokens for AsProcessTableUploadResponseFunction<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let function_name = self.get_function_name();
            let table_name = AsSchemaTable(&self.0.reference_table).get_table_name();
            let full_model_name = AsFullModel(&self.0.reference_table).get_model_name();
            let sync_metadata_model_name =
                AsTableMetadata(&self.0.reference_table).get_struct_name();

            let id_column = &self.0.reference_table.sync_metadata_columns.id.ident;
            let dirty_flag_column = &self
                .0
                .reference_table
                .sync_metadata_columns
                .dirty_flag
                .ident;
            let client_metadata_column = &self
                .0
                .reference_table
                .sync_metadata_columns
                .client_column_sync_metadata
                .ident;

            let column_dirty_clearing = self
                .0
                .reference_table
                .columns
                .iter()
                .filter(|x| {
                    x.client_only_config == ClientOnlyConfig::Disabled
                        && x.mod_on_backend_only_config == BackendOnlyConfig::Disabled
                        && (x.column_type == CarburetorColumnType::Data
                            || x.column_type == CarburetorColumnType::IsDeleted)
                })
                .map(|x| {
                    let col_ident = &x.ident;
                    quote! {
                        if let Some(carburetor::helpers::client_sync_metadata::Metadata {
                            dirty_at: Some(dirty_at), ..
                        }) = data.#col_ident {
                            if dirty_at < cutoff_at {
                                data.#col_ident.get_or_insert_default().dirty_at = None;
                                data.#col_ident.get_or_insert_default().column_last_synced_at = Some(res.last_synced_at);
                            } else {
                                flag = Some(carburetor::helpers::client_sync_metadata::DirtyFlag::Update.to_string());
                            }
                        }
                    }
                })
                .collect::<Vec<_>>();

            tokens.extend(quote! {
                fn #function_name(
                    cutoff_at: carburetor::chrono::DateTimeUtc,
                    responses: Vec<Result<carburetor::models::UploadTableResponseData, carburetor::models::UploadTableResponseError>>,
                    conn: &mut diesel::SqliteConnection,
                ) -> carburetor::error::Result<()> {
                    for response in responses {
                        match response {
                            Ok(res) => {
                                conn.immediate_transaction(|conn| {
                                    let maybe_record = #table_name::table
                                        .select(#full_model_name::as_select())
                                        .find(res.id.clone())
                                        .first(conn)
                                        .optional()?;

                                    if let Some(record) = maybe_record {
                                        let mut metadata = carburetor::helpers::client_sync_metadata::ClientSyncMetadata::<#sync_metadata_model_name>::from(record.#client_metadata_column);

                                        match record.#dirty_flag_column {
                                            Some(ref f) if
                                                f == &carburetor::helpers::client_sync_metadata::DirtyFlag::Insert.to_string() ||
                                                f == &carburetor::helpers::client_sync_metadata::DirtyFlag::Update.to_string() => {
                                                    let mut flag = None;

                                                    if metadata.insert_time.is_some_and(|x| x < cutoff_at) {
                                                        metadata.insert_time = None;
                                                    }

                                                    if let Some(mut data) = metadata.data {
                                                        #(#column_dirty_clearing)*
                                                        metadata.data = Some(data);
                                                    }

                                                    diesel::update(#table_name::table.find(record.#id_column))
                                                        .set((
                                                            #table_name::dsl::#client_metadata_column.eq(carburetor::serde_json::Value::from(metadata)),
                                                            #table_name::dsl::#dirty_flag_column.eq(flag)
                                                        ))
                                                        .execute(conn)?;
                                                }
                                            _ => {}
                                        }
                                    }
                                    Ok(())
                                })
                                .map_err(|e: diesel::result::Error| {
                                    carburetor::error::Error::Unhandled {
                                        message: "error while processing upload response".to_string(),
                                        source: e.into(),
                                    }
                                })?;
                            }
                            Err(e) => {
                                match e.code {
                                    carburetor::models::UploadTableResponseErrorType::RecordAlreadyExists => {
                                        // TODO: handle by changing the ID. Note that this might cause complication when
                                        // foreign key feature is introduced
                                    }
                                    carburetor::models::UploadTableResponseErrorType::RecordNotFound => {
                                        // TODO: handle by creating the record as insert record instead
                                    }
                                    carburetor::models::UploadTableResponseErrorType::Unknown => {
                                        // Nothing to do because we don't know what's happening
                                    }
                                }
                            }
                        }
                    }
                    Ok(())
                }
            });
        }
    }

    pub struct AsProcessUploadResponseFunction<'a>(pub &'a CarburetorSyncGroup);

    impl<'a> ToTokens for AsProcessUploadResponseFunction<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let table_process_functions = self
                .0
                .table_configs
                .iter()
                .map(|x| AsProcessTableUploadResponseFunction(x))
                .collect::<Vec<_>>();

            let upload_response_model_name = AsUploadResponseModel(self.0).get_model_name();

            let table_process_calls = self
                .0
                .table_configs
                .iter()
                .map(|x| {
                    let field = &x.reference_table.ident;
                    let function_name = AsProcessTableUploadResponseFunction(x).get_function_name();
                    quote!(#function_name(cutoff_at, upload_response.#field, &mut conn)?)
                })
                .collect::<Vec<_>>();

            tokens.extend(quote! {
                pub fn store_upload_response(
                    cutoff_at: carburetor::chrono::DateTimeUtc,
                    upload_response: #upload_response_model_name,
                ) -> carburetor::error::Result<()> {
                    use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper};

                    #(#table_process_functions)*

                    let mut conn = carburetor::helpers::get_connection()?;
                    #(#table_process_calls;)*
                    Ok(())
                }
            });
        }
    }
}

#[cfg(feature = "backend")]
mod backend {
    use proc_macro2::TokenStream;
    use quote::{ToTokens, format_ident, quote};
    use syn::Ident;

    use crate::{
        generators::{
            diesel::{
                models::{AsChangesetModel, AsFullModel, backend::AsInsertModel},
                schema::AsSchemaTable,
            },
            upload::models::{AsUploadRequest, AsUploadRequestTable, AsUploadResponseModel},
        },
        parsers::{
            sync_group::{CarburetorSyncGroup, SyncGroupTableConfig},
            table::column::BackendOnlyConfig,
        },
    };

    struct AsProcessTableUploadFunction<'a>(&'a SyncGroupTableConfig);

    impl<'a> AsProcessTableUploadFunction<'a> {
        fn get_function_name(&self) -> Ident {
            format_ident!("process_upload_request_{}", self.0.reference_table.ident)
        }
    }

    impl<'a> ToTokens for AsProcessTableUploadFunction<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let function_name = self.get_function_name();
            let upload_request_table_name = AsUploadRequestTable(self.0).get_model_name();
            let table_name = AsSchemaTable(&self.0.reference_table).get_table_name();
            let full_model_name =
                AsFullModel(&self.0.reference_table).get_model_name_with_prefix("super");
            let insert_model_name =
                AsInsertModel(&self.0.reference_table).get_model_name_with_prefix("super");
            let changeset_model_name =
                AsChangesetModel(&self.0.reference_table).get_model_name_with_prefix("super");
            let id_column = &self.0.reference_table.sync_metadata_columns.id.ident;
            let last_synced_at_column = &self
                .0
                .reference_table
                .sync_metadata_columns
                .last_synced_at
                .ident;
            let backend_server_utc_update_columns = self
                .0
                .reference_table
                .columns
                .iter()
                .filter_map(|x| match x.mod_on_backend_only_config {
                    BackendOnlyConfig::Disabled => None,

                    BackendOnlyConfig::BySqlUtcNow => {
                        let column_name = &x.ident;
                        Some(quote!(super::#table_name::#column_name.eq(diesel::dsl::now)))
                    }
                })
                .collect::<Vec<_>>();

            tokens.extend(quote! {
                fn #function_name(
                    requests: Vec<#upload_request_table_name>,
                    connection: &mut diesel::PgConnection,
                ) -> Vec<
                    Result<
                        carburetor::models::UploadTableResponseData,
                        carburetor::models::UploadTableResponseError,
                    >,
                > {
                    use diesel::{QueryDsl, RunQueryDsl, Insertable, ExpressionMethods};
                    requests
                        .into_iter()
                        .map(|x| {
                            match x {
                                #upload_request_table_name::Insert(data) => {
                                    let insert_data = #insert_model_name::from(data);
                                    let id_to_insert = insert_data.#id_column.clone();
                                    diesel::insert_into(super::#table_name::table)
                                        .values((
                                            &insert_data,
                                            #(#backend_server_utc_update_columns,)*
                                        ))
                                        .get_result(connection)
                                        .map(
                                            |x: #full_model_name| carburetor::models::UploadTableResponseData {
                                                id: x.#id_column,
                                                last_synced_at: x.#last_synced_at_column,
                                            },
                                        )
                                        .map_err(|e| {
                                            let code = match e {
                                                diesel::result::Error::DatabaseError(
                                                    diesel::result::DatabaseErrorKind::UniqueViolation, _) =>
                                                    carburetor::models::UploadTableResponseErrorType::RecordAlreadyExists,
                                                _ => carburetor::models::UploadTableResponseErrorType::Unknown,
                                            };
                                            carburetor::models::UploadTableResponseError {
                                                id: id_to_insert,
                                                code,
                                            }
                                        })
                                }
                                #upload_request_table_name::Update(data) => {
                                    let update_data = #changeset_model_name::from(data);
                                    let id_to_update = update_data.#id_column.clone();
                                    diesel::update(super::#table_name::table.find(&update_data.#id_column))
                                        .set((
                                            &update_data,
                                            #(#backend_server_utc_update_columns,)*
                                        ))
                                        .get_result(connection)
                                        .map(|x: #full_model_name| carburetor::models::UploadTableResponseData {
                                            id: x.#id_column,
                                            last_synced_at: x.#last_synced_at_column,
                                        })
                                        .map_err(|e| {
                                            let code = match e {
                                                diesel::result::Error::NotFound =>
                                                    carburetor::models::UploadTableResponseErrorType::RecordNotFound,
                                                _ => carburetor::models::UploadTableResponseErrorType::Unknown,
                                            };
                                            carburetor::models::UploadTableResponseError {
                                                id: id_to_update,
                                                code,
                                            }
                                        })
                                }
                            }
                        })
                        .collect::<Vec<_>>()
                }
            });
        }
    }

    pub struct AsProcessUploadFunction<'a>(pub &'a CarburetorSyncGroup);

    impl<'a> ToTokens for AsProcessUploadFunction<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let table_process_functions = self
                .0
                .table_configs
                .iter()
                .map(|x| AsProcessTableUploadFunction(x))
                .collect::<Vec<_>>();

            let upload_request_model_name = AsUploadRequest(self.0).get_model_name();
            let upload_response_model_name = AsUploadResponseModel(self.0).get_model_name();
            let field_assignments = self
                .0
                .table_configs
                .iter()
                .map(|x| {
                    let field = &x.reference_table.ident;
                    let function_name = AsProcessTableUploadFunction(x).get_function_name();
                    quote!(#field: #function_name(upload_request.#field, &mut connection))
                })
                .collect::<Vec<_>>();

            tokens.extend(quote! {
                pub fn process_upload_request(
                    upload_request: #upload_request_model_name,
                ) -> carburetor::error::Result<#upload_response_model_name> {

                    #(#table_process_functions)*

                    let mut connection = carburetor::helpers::get_connection()?;

                    Ok(#upload_response_model_name {
                        #(#field_assignments,)*
                    })
                }
            });
        }
    }
}

pub fn generate_upload_sync_group_functions(
    tokens: &mut TokenStream,
    sync_group: &CarburetorSyncGroup,
) {
    #[cfg(feature = "client")]
    {
        use crate::generators::upload::functions::client::{
            AsProcessUploadResponseFunction, AsRetrieveUploadFunction,
        };
        use quote::ToTokens;

        tokens.extend(AsRetrieveUploadFunction(sync_group).to_token_stream());
        tokens.extend(AsProcessUploadResponseFunction(sync_group).to_token_stream());
    }

    #[cfg(feature = "backend")]
    {
        use crate::generators::upload::functions::backend::AsProcessUploadFunction;
        use quote::ToTokens;

        tokens.extend(AsProcessUploadFunction(sync_group).to_token_stream());
    }
}
