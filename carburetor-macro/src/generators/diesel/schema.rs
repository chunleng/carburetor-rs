use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Type, parse_str};

use crate::parsers::table::{
    CarburetorTable, column::CarburetorColumn, postgres_type::DieselPostgresType,
};

struct AsSchemaColumn<'a>(&'a CarburetorColumn);

impl<'a> ToTokens for AsSchemaColumn<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.0.ident;
        let ty = AsSchemaType(&self.0.diesel_type);
        tokens.extend(quote! {
            #name -> #ty
        });
    }
}

struct AsSchemaType<'a>(&'a DieselPostgresType);

impl<'a> ToTokens for AsSchemaType<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty: Type = parse_str(&self.0.to_string()).unwrap();
        tokens.extend(quote! { #ty });
    }
}

pub(crate) fn generate_diesel_table_schema(tokens: &mut TokenStream, table: &CarburetorTable) {
    let table_name = &table.plural_ident;
    let id_column_name = &table.sync_metadata_columns.id.ident;
    let id_column = AsSchemaColumn(&*table.sync_metadata_columns.id);
    let data_columns = table
        .data_columns
        .iter()
        .map(|x| AsSchemaColumn(x))
        .collect::<Vec<_>>();
    let last_synced_at_column = AsSchemaColumn(&*table.sync_metadata_columns.last_synced_at);

    tokens.extend(quote! {
        diesel::table! {
            #table_name (#id_column_name) {
                #id_column,
                #(#data_columns,)*
                #last_synced_at_column,
            }
        }
    });
}
