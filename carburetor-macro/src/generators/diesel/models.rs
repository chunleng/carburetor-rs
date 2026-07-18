use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Ident, Path, Type, parse_quote, parse_str};

use crate::{
    generators::diesel::schema::AsSchemaTable,
    helpers::{TargetType, get_target_type},
    parsers::table::{
        CarburetorTable,
        column::{CarburetorColumn, CarburetorColumnType, ColumnScope, DefaultValue},
        postgres_type::DieselPostgresType,
    },
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

pub struct AsModelType<'a>(pub &'a DieselPostgresType);

impl<'a> ToTokens for AsModelType<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ty: Type = parse_str(&self.0.get_model_type_string()).unwrap();
        tokens.extend(quote! { #ty });
    }
}

pub(crate) struct AsFullModel<'a>(pub(crate) &'a CarburetorTable);

impl<'a> AsFullModel<'a> {
    pub(crate) fn get_model_name(&self) -> Ident {
        parse_str::<Ident>(&format!(
            "Full{}",
            self.0.ident.to_string().to_upper_camel_case()
        ))
        .unwrap()
    }
    pub(crate) fn get_model_name_with_prefix(&self, prefix: &str) -> Path {
        let model_name = self.get_model_name();
        let prefix: Path = parse_str(prefix).unwrap();
        parse_quote!(#prefix::#model_name)
    }
}

impl<'a> ToTokens for AsFullModel<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let full_model_name = self.get_model_name();

        let columns = self
            .0
            .columns
            .iter()
            .filter_map(|x| {
                let name = &x.ident;
                let ty = AsModelType(&x.diesel_type);
                match get_target_type() {
                    TargetType::Backend => match x.column_scope {
                        ColumnScope::ClientOnly => None,
                        _ => Some(quote!(pub #name: #ty)),
                    },
                    TargetType::Client => match x.column_scope {
                        ColumnScope::ModOnBackendOnly => Some(quote!(pub #name: Option<#ty> )),
                        _ => Some(quote!(pub #name: #ty)),
                    },
                }
            })
            .collect::<Vec<_>>();
        let diesel_table = AsDieselTable {
            table: &self.0,
            prefix: None,
        };
        let derive_header = quote!(#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, diesel::Queryable, diesel::Selectable)]);

        tokens.extend(quote! {
            #derive_header
            #diesel_table
            pub struct #full_model_name {
                #(#columns,)*
            }
        });
    }
}

pub(crate) struct AsInsertModel<'a>(pub(crate) &'a CarburetorTable);

impl<'a> AsInsertModel<'a> {
    pub(crate) fn get_model_name(&self) -> Ident {
        parse_str::<Ident>(&format!(
            "Insertable{}",
            self.0.ident.to_string().to_upper_camel_case()
        ))
        .unwrap()
    }
    pub(crate) fn get_model_name_with_prefix(&self, prefix: &str) -> Path {
        let model_name = self.get_model_name();
        let prefix: Path = parse_str(prefix).unwrap();
        parse_quote!(#prefix::#model_name)
    }
}

impl<'a> ToTokens for AsInsertModel<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = self.get_model_name();

        let columns = self
            .0
            .columns
            .iter()
            .filter_map(|x| {
                let name = &x.ident;
                let ty = AsModelType(&x.diesel_type);
                match get_target_type() {
                    TargetType::Backend => match x.column_scope {
                        ColumnScope::Both => {
                            let is_sql = match x.default_value {
                                #[cfg(feature = "migration")]
                                Some(DefaultValue::Sql(_)) => true,
                                #[cfg(not(feature = "migration"))]
                                Some(DefaultValue::Sql) => true,
                                _ => false,
                            };
                            if is_sql {
                                Some(quote!(pub #name: Option<#ty>))
                            } else {
                                Some(quote!(pub #name: #ty))
                            }
                        }
                        _ => None,
                    },
                    TargetType::Client => match x.column_scope {
                        ColumnScope::ModOnBackendOnly => Some(quote!(pub #name: Option<#ty>)),
                        _ => {
                            let is_sql = match x.default_value {
                                #[cfg(feature = "migration")]
                                Some(DefaultValue::Sql(_)) => true,
                                #[cfg(not(feature = "migration"))]
                                Some(DefaultValue::Sql) => true,
                                _ => false,
                            };
                            if is_sql {
                                Some(quote!(pub #name: Option<#ty>))
                            } else {
                                Some(quote!(pub #name: #ty))
                            }
                        }
                    },
                }
            })
            .collect::<Vec<_>>();

        let diesel_table = AsDieselTable {
            table: self.0,
            prefix: None,
        };

        tokens.extend(quote! {
            #[derive(Debug, Clone, diesel::Insertable)]
            #diesel_table
            pub struct #model_name {
                #(#columns,)*
            }
        });
    }
}

pub(crate) struct AsChangesetModel<'a>(pub(crate) &'a CarburetorTable);

impl<'a> AsChangesetModel<'a> {
    pub(crate) fn get_model_name(&self) -> Ident {
        parse_str::<Ident>(&format!(
            "Changeset{}",
            self.0.ident.to_string().to_upper_camel_case()
        ))
        .unwrap()
    }
    pub(crate) fn get_model_name_with_prefix(&self, prefix: &str) -> Path {
        let model_name = self.get_model_name();
        let prefix: Path = parse_str(prefix).unwrap();
        parse_quote!(#prefix::#model_name)
    }
}

impl<'a> ToTokens for AsChangesetModel<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let table = self.0;
        let update_model_name = self.get_model_name();
        let columns = table
            .columns
            .iter()
            .filter_map(|x| {
                let name = &x.ident;
                let ty = AsModelType(&x.diesel_type);

                match get_target_type() {
                    TargetType::Backend => {
                        match (&x.column_type, &x.column_scope, &x.is_immutable) {
                            (CarburetorColumnType::Id, _, _) => Some(quote!(pub #name: #ty)),
                            (_, ColumnScope::ClientOnly, _) | (_, _, true) => None,
                            (_, _, _) => Some(quote!(pub #name: Option<#ty>)),
                        }
                    }
                    TargetType::Client => match (&x.column_type, &x.column_scope) {
                        (CarburetorColumnType::Id, _) => Some(quote!(pub #name: #ty)),
                        (_, ColumnScope::ModOnBackendOnly) => {
                            Some(quote!(pub #name: Option<Option<#ty>>))
                        }
                        (_, _) => Some(quote!(pub #name: Option<#ty>)),
                    },
                }
            })
            .collect::<Vec<_>>();
        let diesel_table = AsDieselTable {
            table,
            prefix: None,
        };

        tokens.extend(quote! {
            #[derive(Debug, Clone, diesel::AsChangeset)]
            #diesel_table
            pub struct #update_model_name {
                #(#columns,)*
            }
        });
    }
}

pub(crate) struct AsDieselTable<'a> {
    pub(crate) table: &'a CarburetorTable,
    pub(crate) prefix: Option<&'a str>,
}

impl<'a> ToTokens for AsDieselTable<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let table_name = AsSchemaTable(self.table).get_table_name();
        let table_path = match self.prefix {
            Some(x) => {
                let x: Path = parse_str(x).unwrap();
                quote!(#x::#table_name)
            }
            None => {
                quote!(#table_name)
            }
        };
        let diesel_backend: Path;
        match get_target_type() {
            TargetType::Backend => {
                diesel_backend = parse_quote!(diesel::pg::Pg);
            }
            TargetType::Client => {
                diesel_backend = parse_quote!(diesel::sqlite::Sqlite);
            }
        }

        tokens.extend(quote! {
            #[diesel(table_name = #table_path)]
            #[diesel(check_for_backend(#diesel_backend))]
        });
    }
}

pub(crate) fn generate_diesel_model(tokens: &mut TokenStream, table: &CarburetorTable) {
    let new_model = AsFullModel(&table);
    let update_model = AsChangesetModel(&table);
    let insert_model = AsInsertModel(&table);

    tokens.extend(quote! {
        #new_model
        #update_model
        #insert_model
    });
}
