use std::{cell::RefCell, rc::Rc};

use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{ExprField, Ident, Type, parse_str};

use crate::parsers::{sync_group::CarburetorSyncGroup, table::CarburetorTable};

struct AsResponseFieldValue<'a>(&'a Rc<RefCell<CarburetorTable>>);

impl<'a> ToTokens for AsResponseFieldValue<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let field_name;
        let function_name;
        let function_argument;
        {
            let table = self.0.borrow();
            field_name = table.ident.clone();
            function_name = parse_str::<Ident>(&format!("download_{}", &table.ident)).unwrap();
            function_argument =
                parse_str::<ExprField>(&format!("request.{}_offset", &table.ident)).unwrap();
        }
        tokens.extend(quote! {
            #field_name: #function_name(#function_argument)?
        });
    }
}
struct AsDownloadFunction<'a>(&'a Rc<RefCell<CarburetorTable>>);

impl<'a> ToTokens for AsDownloadFunction<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let function_name;
        let model_name;
        let table_name;
        let last_synced_at_column_name;

        {
            let table = self.0.borrow();
            function_name = parse_str::<Ident>(&format!("download_{}", &table.ident)).unwrap();
            model_name =
                parse_str::<Ident>(&table.ident.to_string().to_upper_camel_case()).unwrap();
            table_name = table.plural_ident.clone();
            last_synced_at_column_name = table.sync_metadata_columns.last_synced_at.ident.clone();
        }

        tokens.extend(quote! {
            fn #function_name(
                offset: Option<carburetor::chrono::DateTimeUtc>,
            ) -> carburetor::error::Result<carburetor::backend::models::DownloadSyncResponse<#model_name>>
            {
                use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
                let mut conn = carburetor::backend::helpers::get_connection()?;

                let process_time = carburetor::backend::helpers::get_utc_now();
                let mut query = #table_name::table
                    .select(#model_name::as_select())
                    .filter(#table_name::dsl::#last_synced_at_column_name.le(process_time))
                    .into_boxed();

                if let Some(offset) = offset {
                    query = query.filter(#table_name::dsl::#last_synced_at_column_name.gt(offset));
                }

                Ok(carburetor::backend::models::DownloadSyncResponse {
                    last_synced_at: process_time,
                    data: query
                        .load(&mut conn)
                        .map_err(|e| carburetor::error::Error::Unhandled {
                            message: "Query execution failed".to_string(),
                            source: e.into(),
                        })?,
                })
            }
        });
    }
}

pub(crate) fn generate_download_sync_group_functions(
    token: &mut TokenStream,
    sync_group: &CarburetorSyncGroup,
) {
    let function_name = parse_str::<Ident>(&format!("download_{}", sync_group.name)).unwrap();
    let request_model_name = parse_str::<Type>(&format!(
        "Download{}Request",
        sync_group.name.to_string().to_upper_camel_case()
    ))
    .unwrap();
    let response_model_name = parse_str::<Type>(&format!(
        "Download{}Response",
        sync_group.name.to_string().to_upper_camel_case()
    ))
    .unwrap();

    let table_download_function = sync_group
        .tables
        .iter()
        .map(|x| AsDownloadFunction(x))
        .collect::<Vec<_>>();

    let table_response_field_values = sync_group
        .tables
        .iter()
        .map(|x| AsResponseFieldValue(x))
        .collect::<Vec<_>>();

    token.extend(quote! {
        fn #function_name(
            request: #request_model_name,
        ) -> carburetor::error::Result<#response_model_name> {
            #(#table_download_function)*
            Ok(#response_model_name {
                #(#table_response_field_values,)*
            })
        }
    })
}
