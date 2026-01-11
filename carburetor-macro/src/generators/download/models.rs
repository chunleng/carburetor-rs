use std::rc::Rc;

use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Ident, Type, parse_quote, parse_str};

use crate::{
    generators::diesel::models::AsModelType,
    parsers::{
        sync_group::CarburetorSyncGroup,
        table::{CarburetorTable, column::ClientOnlyConfig},
    },
};

struct AsRequestField<'a>(&'a Rc<CarburetorTable>);

impl<'a> ToTokens for AsRequestField<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let field_name =
            parse_str::<Type>(&format!("{}_offset", self.0.ident.to_string())).unwrap();
        tokens.extend(quote! {
            pub #field_name: Option<carburetor::chrono::DateTimeUtc>
        });
    }
}

pub struct AsResponseField<'a>(pub &'a CarburetorSyncGroup, pub &'a CarburetorTable);

impl<'a> AsResponseField<'a> {
    pub fn get_field_name(&self) -> Ident {
        self.1.ident.clone()
    }
}

impl<'a> ToTokens for AsResponseField<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let field_name = self.get_field_name();
        let model_name = AsDownloadResponseTableModel(self.0, self.1).get_model_name();

        tokens.extend(quote! {
            pub #field_name: carburetor::models::DownloadTableResponse<#model_name>
        });
    }
}

#[cfg(feature = "client")]
mod client {
    use crate::{
        generators::diesel::models::{AsChangesetModel, AsFullModel},
        parsers::table::column::{BackendOnlyConfig, CarburetorColumnType, ClientOnlyConfig},
    };

    use super::*;

    pub struct AsFromModelToNewTableModel<'a> {
        pub model_name: &'a Ident,
        pub table: &'a CarburetorTable,
    }
    impl<'a> ToTokens for AsFromModelToNewTableModel<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let model_name = self.model_name;
            let diesel_full_model = AsFullModel(&self.table).get_model_name();

            let columns = self
                .table
                .columns
                .iter()
                .map(|x| {
                    let column_name = &x.ident;
                    match &x.client_only_config {
                        // Cilent only columns will not have a value at this point because download
                        // model (Backend). However, the value might not be used when it is a full
                        // update instead of a insert. Usage of the value is up to the
                        // sync_local_db script to handle.
                        ClientOnlyConfig::Enabled { default_value } => {
                            quote!(#column_name: #default_value)
                        }
                        ClientOnlyConfig::Disabled => match x.mod_on_backend_only_config {
                            BackendOnlyConfig::Disabled => {
                                quote!(#column_name: value.#column_name)
                            }
                            BackendOnlyConfig::BySqlUtcNow => {
                                quote!(#column_name: Some(value.#column_name))
                            }
                        },
                    }
                })
                .collect::<Vec<_>>();
            tokens.extend(quote! {
                impl From<#model_name> for #diesel_full_model {
                    fn from(value: #model_name) -> Self {
                        Self {
                            #(#columns,)*
                        }
                    }
                }
            })
        }
    }

    pub struct AsFromModelToUpdateTableModel<'a> {
        pub model_name: &'a Ident,
        pub table: &'a CarburetorTable,
    }
    impl<'a> ToTokens for AsFromModelToUpdateTableModel<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let model_name = self.model_name;
            let diesel_changeset_model = AsChangesetModel(&self.table).get_model_name();

            let columns = self
                .table
                .columns
                .iter()
                .map(|x| {
                    let column_name = &x.ident;
                    match (&x.column_type, &x.client_only_config) {
                        (CarburetorColumnType::Id, _) => quote!(#column_name: value.#column_name),
                        // When updating from backend, the backend will be left empty by default
                        // because ClientOnlyConfig should not be updatable via download sync
                        (_, ClientOnlyConfig::Enabled { .. }) => {
                            quote!(#column_name: None)
                        }
                        (_, ClientOnlyConfig::Disabled) => match x.mod_on_backend_only_config {
                            BackendOnlyConfig::Disabled => {
                                quote!(#column_name: Some(value.#column_name))
                            }
                            BackendOnlyConfig::BySqlUtcNow => {
                                quote!(#column_name: Some(Some(value.#column_name)))
                            }
                        },
                    }
                })
                .collect::<Vec<_>>();
            tokens.extend(quote! {
                impl From<#model_name> for #diesel_changeset_model {
                    fn from(value: #model_name) -> Self {
                        Self {
                            #(#columns,)*
                        }
                    }
                }
            })
        }
    }
}

pub(crate) struct AsDownloadResponseTableModel<'a>(
    pub(crate) &'a CarburetorSyncGroup,
    pub(crate) &'a CarburetorTable,
);

impl<'a> AsDownloadResponseTableModel<'a> {
    pub fn get_model_name(&self) -> Ident {
        Ident::new(
            &format!(
                "DownloadUpdate{}",
                self.1.ident.to_string().to_upper_camel_case()
            ),
            self.1.ident.span(),
        )
    }

    pub fn get_type(&self) -> Type {
        let model_name = self.get_model_name();
        parse_quote!(#model_name)
    }
}

impl<'a> ToTokens for AsDownloadResponseTableModel<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let table = self.1;
        let model_name = &self.get_model_name();
        let columns = table
            .columns
            .iter()
            .filter_map(|x| match x.client_only_config {
                // Backend database will not have any information as this is client-only.
                ClientOnlyConfig::Enabled { .. } => None,
                ClientOnlyConfig::Disabled => {
                    let name = &x.ident;
                    let ty = AsModelType(&x.diesel_type);

                    Some(quote!(pub #name: #ty))
                }
            })
            .collect::<Vec<_>>();

        let attribute;
        let diesel_table;
        let from_model_to_new_table_model;
        let from_model_to_update_table_model;
        #[cfg(feature = "backend")]
        {
            attribute = quote! {
                #[derive(Debug, Clone, diesel::Queryable, diesel::Selectable, serde::Serialize)]
            };
            diesel_table = crate::generators::diesel::models::AsDieselTable {
                table,
                prefix: Some("super"),
            };
            from_model_to_new_table_model = quote! {};
            from_model_to_update_table_model = quote! {};
        }
        #[cfg(feature = "client")]
        {
            attribute = quote! {
                #[derive(Debug, Clone, serde::Deserialize)]
            };
            diesel_table = quote! {};
            from_model_to_new_table_model =
                client::AsFromModelToNewTableModel { model_name, table };
            from_model_to_update_table_model =
                client::AsFromModelToUpdateTableModel { model_name, table };
        }

        tokens.extend(quote! {
            #attribute
            #diesel_table
            pub struct #model_name {
                #(#columns,)*
            }

            #from_model_to_new_table_model
            #from_model_to_update_table_model
        });
    }
}

pub(crate) struct AsDownloadResponseModel<'a>(pub(crate) &'a CarburetorSyncGroup);

impl<'a> AsDownloadResponseModel<'a> {
    pub(crate) fn get_model_name(&self) -> Ident {
        parse_quote!(DownloadResponse)
    }

    pub fn get_response_field_by_table<'b>(
        &'b self,
        table: &'b CarburetorTable,
    ) -> AsResponseField<'b> {
        AsResponseField(self.0, table)
    }
}

impl<'a> ToTokens for AsDownloadResponseModel<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = self.get_model_name();
        let fields = self
            .0
            .table_configs
            .iter()
            .map(|x| self.get_response_field_by_table(&x.reference_table))
            .collect::<Vec<_>>();

        tokens.extend(quote! {
            #[derive(Debug, Clone)]
            pub struct #model_name {
                #(#fields,)*
            }
        });
    }
}

pub struct AsDownloadRequestModel<'a>(pub &'a CarburetorSyncGroup);

impl<'a> AsDownloadRequestModel<'a> {
    pub fn get_model_name(&self) -> Ident {
        Ident::new("DownloadRequest", self.0.name.span())
    }
}

impl<'a> ToTokens for AsDownloadRequestModel<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let request_model_name = self.get_model_name();
        let request_fields = self
            .0
            .table_configs
            .iter()
            .map(|x| AsRequestField(&x.reference_table))
            .collect::<Vec<_>>();
        tokens.extend(quote! {
            #[derive(Debug, Clone, Default)]
            pub struct #request_model_name {
                #(#request_fields,)*
            }
        });
    }
}

pub(crate) fn generate_download_sync_group_models(
    tokens: &mut TokenStream,
    sync_group: &CarburetorSyncGroup,
) {
    let request_model = AsDownloadRequestModel(sync_group);
    let response_model = AsDownloadResponseModel(sync_group);
    let response_table_models = sync_group
        .table_configs
        .iter()
        .map(|x| AsDownloadResponseTableModel(sync_group, &x.reference_table))
        .collect::<Vec<_>>();

    tokens.extend(quote! {
        #(#response_table_models)*
        #response_model
        #request_model
    });
}
