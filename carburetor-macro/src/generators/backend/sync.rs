use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::Result;

use crate::{CarburetorArgs, TableDetail};

pub(crate) fn generate_sync_functions(
    args: &CarburetorArgs,
    table: &TableDetail,
) -> Result<TokenStream> {
    let table_name = &args.table_name;
    let function_name = Ident::new(&format!("download_{}_data", table_name), Span::call_site());
    let last_synced_at_column_name = &table.sync_metadata_columns.last_sync_at.ident;
    let select_model = &table.ident;

    Ok(quote! {
        pub fn #function_name(last_synced_at: Option<carburetor::chrono::DateTimeUtc>) -> carburetor::error::Result<carburetor::backend::models::DownloadSyncResponse<#select_model>> {
            use diesel::{SelectableHelper, QueryDsl, RunQueryDsl, ExpressionMethods};
            let mut conn = carburetor::backend::helpers::get_connection()?;

            let process_time = carburetor::backend::helpers::get_utc_now();
            let mut query = #table_name::table
                .select(#select_model::as_select())
                .filter(#table_name::dsl::#last_synced_at_column_name.le(process_time))
                .into_boxed();

            if let Some(last_synced_at) = last_synced_at {
                query = query.filter(#table_name::dsl::#last_synced_at_column_name.gt(last_synced_at));
            }

            Ok(carburetor::backend::models::DownloadSyncResponse {
                last_sync_at: process_time,
                data: query
                    .load(&mut conn)
                    .map_err(|e| carburetor::error::Error::Unhandled {
                        message: "Query execution failed".to_string(),
                        source: e.into(),
                    })?,
            })
        }
    })
}
