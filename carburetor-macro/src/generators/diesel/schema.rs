use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Ident, Path, Type, parse_quote, parse_str};

use crate::parsers::table::{CarburetorTable, postgres_type::DieselPostgresType};

struct AsSchemaType<'a>(&'a DieselPostgresType);

impl<'a> ToTokens for AsSchemaType<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        #[cfg(feature = "backend")]
        let ty: Type = parse_str(&self.0.to_string()).unwrap();
        #[cfg(feature = "client")]
        let ty: Type = parse_str(&self.0.get_diesel_sqlite_string()).unwrap();
        tokens.extend(quote! { #ty });
    }
}

pub struct AsSchemaTable<'a>(pub &'a CarburetorTable);

impl<'a> AsSchemaTable<'a> {
    pub fn get_table_name(&self) -> Ident {
        self.0.plural_ident.clone()
    }

    pub fn get_table_name_with_prefix(&self, prefix: &str) -> Path {
        let table_name = self.get_table_name();
        let prefix: Path = parse_str(prefix).unwrap();
        parse_quote!(#prefix::#table_name)
    }
}

impl<'a> ToTokens for AsSchemaTable<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let table_name = self.get_table_name();
        let id_column_name = &self.0.sync_metadata_columns.id.ident;
        let data_columns = self
            .0
            .columns
            .iter()
            .filter_map(|x| {
                #[cfg(feature = "backend")]
                {
                    use crate::parsers::table::column::ClientOnlyConfig;
                    match x.client_only_config {
                        ClientOnlyConfig::Enabled { .. } => None,
                        ClientOnlyConfig::Disabled => {
                            let name = &x.ident;
                            let ty = AsSchemaType(&x.diesel_type);
                            Some(quote!(#name -> #ty))
                        }
                    }
                }
                #[cfg(feature = "client")]
                {
                    use crate::parsers::table::column::BackendOnlyConfig;

                    let name = &x.ident;
                    let ty = AsSchemaType(&x.diesel_type);
                    match x.mod_on_backend_only_config {
                        BackendOnlyConfig::Disabled => {
                            return Some(quote!(#name -> #ty));
                        }
                        BackendOnlyConfig::BySqlUtcNow => {
                            return Some(quote!(#name -> Nullable<#ty>));
                        }
                    }
                }
            })
            .collect::<Vec<_>>();
        tokens.extend(quote! {
            #table_name (#id_column_name) {
                #(#data_columns,)*
            }
        });
    }
}

pub(crate) fn generate_diesel_table_schema(tokens: &mut TokenStream, table: &CarburetorTable) {
    let schema_table = AsSchemaTable(table);

    tokens.extend(quote! {
        diesel::table! {
            #schema_table
        }
    });
}
