use proc_macro2::TokenStream;

use crate::parsers::sync_group::CarburetorSyncGroup;

#[cfg(feature = "client")]
mod client {
    use proc_macro2::TokenStream;
    use quote::{ToTokens, format_ident, quote};

    use crate::{
        generators::{
            diesel::schema::AsSchemaTable, download::models::AsDownloadRequestModel,
        },
        parsers::sync_group::CarburetorSyncGroup,
    };

    pub struct AsRetrieveDownloadRequestFunction<'a>(pub &'a CarburetorSyncGroup);

    impl<'a> ToTokens for AsRetrieveDownloadRequestFunction<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let download_request_model_name = AsDownloadRequestModel(self.0).get_model_name();

            let field_assignments = self
                .0
                .table_configs
                .iter()
                .map(|x| {
                    let field_name = format_ident!("{}_offset", x.reference_table.ident);
                    let table_name_str = AsSchemaTable(&x.reference_table).get_table_name().to_string();
                    quote!(#field_name: offsets.get(#table_name_str).cloned())
                })
                .collect::<Vec<_>>();

            tokens.extend(quote! {
                pub fn retrieve_download_request()
                -> carburetor::error::Result<Option<#download_request_model_name>> {
                    let mut conn = carburetor::helpers::get_connection()?;
                    let offsets = carburetor::helpers::carburetor_offset::retrieve_offsets(&mut conn)?;

                    Ok(if offsets.is_empty() {
                        None
                    } else {
                        Some(#download_request_model_name {
                            #(#field_assignments,)*
                        })
                    })
                }
            });
        }
    }
}

#[cfg(feature = "backend")]
mod backend {
    use std::rc::Rc;

    use proc_macro2::TokenStream;
    use quote::{ToTokens, quote};
    use syn::{ExprField, Ident, Path, Type, parse_quote, parse_str};

    use crate::{
        generators::{
            diesel::schema::AsSchemaTable,
            download::models::{
                AsDownloadRequestModel, AsDownloadResponseModel, AsDownloadResponseTableModel,
            },
        },
        parsers::{sync_group::CarburetorSyncGroup, table::CarburetorTable},
    };

    struct AsResponseFieldValue<'a>(&'a Rc<CarburetorTable>);

    impl<'a> ToTokens for AsResponseFieldValue<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let table = self.0;
            let field_name = table.ident.clone();
            let function_name = parse_str::<Ident>(&format!("download_{}", &table.ident)).unwrap();
            let function_argument =
                parse_str::<ExprField>(&format!("request.{}_offset", &table.ident)).unwrap();

            tokens.extend(quote! {
                #field_name: #function_name(#function_argument, clean_download)?
            });
        }
    }

    struct AsDownloadFunction<'a>(&'a CarburetorSyncGroup, &'a CarburetorTable);

    impl<'a> ToTokens for AsDownloadFunction<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let table = self.1;
            let function_name = parse_str::<Ident>(&format!("download_{}", &table.ident)).unwrap();
            let model_name = AsDownloadResponseTableModel(&self.0, &table).get_model_name();
            let table_name = AsSchemaTable(table).get_table_name_with_prefix("super");
            let last_synced_at_column_name = table.sync_metadata_columns.last_synced_at.ident.clone();
            let is_deleted_column_name = table.sync_metadata_columns.is_deleted.ident.clone();

            let download_sync_response: Path = parse_quote! {carburetor::models::DownloadTableResponse};
            let download_sync_response_data: Path =
                parse_quote! {carburetor::models::DownloadTableResponseData};

            let return_type: Type = parse_quote! {
                carburetor::error::Result<
                    #download_sync_response<#model_name>
                >
            };

            tokens.extend(quote! {
                fn #function_name(
                    offset: Option<carburetor::chrono::DateTimeUtc>,
                    clean_download: bool
                ) -> #return_type
                {
                    use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
                    let mut conn = carburetor::helpers::get_connection()?;

                    let process_time = carburetor::helpers::get_utc_now();
                    let mut query = #table_name::table
                        .select(#model_name::as_select())
                        .filter(#table_name::dsl::#last_synced_at_column_name.le(process_time))
                        .into_boxed();

                    if let Some(offset) = offset {
                        query = query.filter(#table_name::dsl::#last_synced_at_column_name.gt(offset));
                    }

                    if clean_download {
                        query = query.filter(#table_name::dsl::#is_deleted_column_name.eq(false));
                    }

                    Ok(#download_sync_response {
                        cutoff_at: process_time,
                        data: query
                            .load(&mut conn)
                            .map_err(|e| carburetor::error::Error::Unhandled {
                                message: "Query execution failed".to_string(),
                                source: e.into(),
                            })?
                            .into_iter()
                            .map(|x| { #download_sync_response_data::Update(x) })
                            .collect::<Vec<_>>(),
                    })
                }
            });
        }
    }

    pub struct AsProcessDownloadRequestFunction<'a>(pub &'a CarburetorSyncGroup);

    impl<'a> ToTokens for AsProcessDownloadRequestFunction<'a> {
        fn to_tokens(&self, token: &mut TokenStream) {
            let function_name = parse_str::<Ident>("process_download_request").unwrap();
            let request_model_name = AsDownloadRequestModel(self.0).get_model_name();
            let response_model_name = AsDownloadResponseModel(self.0).get_model_name();

            let table_download_function = self
                .0
                .table_configs
                .iter()
                .map(|x| AsDownloadFunction(&self.0, &x.reference_table))
                .collect::<Vec<_>>();

            let table_response_field_values = self
                .0
                .table_configs
                .iter()
                .map(|x| AsResponseFieldValue(&x.reference_table))
                .collect::<Vec<_>>();

            token.extend(quote! {
                pub fn #function_name(
                    request: Option<#request_model_name>,
                ) -> carburetor::error::Result<#response_model_name> {
                    let clean_download = request.is_none();
                    let request = request.unwrap_or_default();
                    #(#table_download_function)*
                    Ok(#response_model_name {
                        #(#table_response_field_values,)*
                    })
                }
            })
        }
    }
}

pub fn generate_download_sync_group_functions(
    tokens: &mut TokenStream,
    sync_group: &CarburetorSyncGroup,
) {
    #[cfg(feature = "client")]
    {
        use crate::generators::download::functions::client::AsRetrieveDownloadRequestFunction;
        use quote::ToTokens;

        tokens.extend(AsRetrieveDownloadRequestFunction(sync_group).to_token_stream());
    }

    #[cfg(feature = "backend")]
    {
        use crate::generators::download::functions::backend::AsProcessDownloadRequestFunction;
        use quote::ToTokens;

        tokens.extend(AsProcessDownloadRequestFunction(sync_group).to_token_stream());
    }
}
