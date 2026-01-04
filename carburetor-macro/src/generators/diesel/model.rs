use crate::{
    CarburetorArgs,
    parsers::input::{DataColumn, TableDetail},
};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Ident;

fn generate_model_field_token_stream(col: &DataColumn) -> TokenStream2 {
    let field_vis = &col.vis;
    let field_name = &col.ident;
    let field_ty = &col.ty;
    quote! {
        #field_vis #field_name: #field_ty
    }
}

fn generate_changset_model_field_token_stream(col: &DataColumn) -> TokenStream2 {
    let field_vis = &col.vis;
    let field_name = &col.ident;
    let field_ty = &col.ty;
    quote! {
        #field_vis #field_name: Option<#field_ty>
    }
}

pub(crate) fn generate_diesel_models(table: &TableDetail, config: &CarburetorArgs) -> TokenStream2 {
    let vis = &table.vis;
    let name = &table.ident;
    let update_name = Ident::new(&format!("Update{}", table.ident), table.ident.span());
    let table_name = &config.table_name;

    let id_column = generate_model_field_token_stream(&table.id_column);
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
        #vis struct #name {
            #id_column,
            #(#data_columns),*
        }
        #[derive(Debug, Clone, diesel::AsChangeset)]
        #[diesel(table_name = #table_name)]
        #[diesel(check_for_backend(diesel::pg::Pg))]
        #vis struct #update_name {
            #id_column,
            #(#changeset_data_columns),*
        }
    }
    .into()
}
