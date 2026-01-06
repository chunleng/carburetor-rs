use crate::{CarburetorTable, parsers::CarburetorColumn};
use proc_macro2::TokenStream;
use quote::quote;

fn generate_model_field_token_stream(col: &CarburetorColumn) -> TokenStream {
    let field_vis = &col.model_field_vis;
    let field_name = &col.ident;
    let field_ty = &col.model_ty;
    quote! {
        #field_vis #field_name: #field_ty
    }
}

fn generate_changset_model_field_token_stream(col: &CarburetorColumn) -> TokenStream {
    let field_vis = &col.model_field_vis;
    let field_name = &col.ident;
    let field_ty = &col.model_ty;
    quote! {
        #field_vis #field_name: Option<#field_ty>
    }
}

pub(crate) fn generate_diesel_models(table: &CarburetorTable) -> TokenStream {
    let model_vis = &table.model_vis;
    let table_name = table.get_table_name();
    let model_name = &table.model_id;
    let update_model_name = table.get_update_model_name();

    let id_column = generate_model_field_token_stream(&*table.sync_metadata_columns.id);
    let last_synced_at_column =
        generate_model_field_token_stream(&*table.sync_metadata_columns.last_synced_at);
    let data_columns: Vec<_> = table
        .data_columns
        .iter()
        .map(generate_model_field_token_stream)
        .collect();
    let changeset_data_columns: Vec<_> = table
        .data_columns
        .iter()
        .map(generate_changset_model_field_token_stream)
        .collect();

    quote! {
        #[derive(Debug, Clone, diesel::Queryable, diesel::Selectable, diesel::Insertable)]
        #[diesel(table_name = #table_name)]
        #[diesel(check_for_backend(diesel::pg::Pg))]
        #model_vis struct #model_name {
            #id_column,
            #(#data_columns,)*
            #last_synced_at_column,
        }
        #[derive(Debug, Clone, diesel::AsChangeset)]
        #[diesel(table_name = #table_name)]
        #[diesel(check_for_backend(diesel::pg::Pg))]
        #model_vis struct #update_model_name {
            #id_column,
            #(#changeset_data_columns,)*
            #last_synced_at_column,
        }
    }
    .into()
}
