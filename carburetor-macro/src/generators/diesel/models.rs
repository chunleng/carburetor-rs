use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Ident, Type, parse_str};

use crate::parsers::table::{
    CarburetorTable, column::CarburetorColumn, postgres_type::DieselPostgresType,
};

struct AsModelChangesetColumn<'a>(&'a CarburetorColumn);

impl<'a> ToTokens for AsModelChangesetColumn<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.0.ident;
        let ty = AsModelType(&self.0.diesel_type);
        tokens.extend(quote! {
            pub #name: Option<#ty>
        });
    }
}
struct AsModelColumn<'a>(&'a CarburetorColumn);

impl<'a> ToTokens for AsModelColumn<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.0.ident;
        let ty = AsModelType(&self.0.diesel_type);
        tokens.extend(quote! {
            pub #name: #ty
        });
    }
}

struct AsModelType<'a>(&'a DieselPostgresType);

impl<'a> ToTokens for AsModelType<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty: Type = parse_str(&self.0.get_model_type_string()).unwrap();
        tokens.extend(quote! { #ty });
    }
}

pub(crate) fn generate_diesel_model(tokens: &mut TokenStream, table: &CarburetorTable) {
    let table_name = &table.plural_ident;
    let model_name = parse_str::<Ident>(&table.ident.to_string().to_upper_camel_case()).unwrap();
    let update_model_name = parse_str::<Ident>(&format!(
        "Update{}",
        table.ident.to_string().to_upper_camel_case()
    ))
    .unwrap();
    let id_column = AsModelColumn(&*table.sync_metadata_columns.id);
    let data_columns = table
        .data_columns
        .iter()
        .map(|x| AsModelColumn(x))
        .collect::<Vec<_>>();
    let changeset_data_columns = table
        .data_columns
        .iter()
        .map(|x| AsModelChangesetColumn(x))
        .collect::<Vec<_>>();
    let last_synced_at_column = AsModelColumn(&*table.sync_metadata_columns.last_synced_at);
    let changeset_last_synced_at_column =
        AsModelChangesetColumn(&*table.sync_metadata_columns.last_synced_at);

    tokens.extend(quote! {
        #[derive(Debug, Clone, diesel::Queryable, diesel::Selectable, diesel::Insertable)]
        #[diesel(table_name = #table_name)]
        #[diesel(check_for_backend(diesel::pg::Pg))]
        pub struct #model_name {
            #id_column,
            #(#data_columns,)*
            #last_synced_at_column,
        }
        #[derive(Debug, Clone, diesel::AsChangeset)]
        #[diesel(table_name = #table_name)]
        #[diesel(check_for_backend(diesel::pg::Pg))]
        pub struct #update_model_name {
            #id_column,
            #(#changeset_data_columns,)*
            #changeset_last_synced_at_column,
        }
    });
}
