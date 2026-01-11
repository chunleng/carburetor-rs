use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::Ident;

use crate::{
    generators::diesel::models::{AsChangesetModel, AsFullModel, AsModelType},
    parsers::{
        sync_group::{CarburetorSyncGroup, SyncGroupTableConfig},
        table::column::{BackendOnlyConfig, CarburetorColumnType, ClientOnlyConfig},
    },
};

pub struct AsLocalInsertModel<'a>(pub &'a SyncGroupTableConfig);

impl<'a> ToTokens for AsLocalInsertModel<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = self.get_model_name();
        let fields = self.0.reference_table.columns.iter().filter_map(|x| {
            if x.column_type == CarburetorColumnType::Data
                && x.mod_on_backend_only_config == BackendOnlyConfig::Disabled
            {
                let field_name = &x.ident;
                let field_type = AsModelType(&x.diesel_type);
                Some(quote! {
                    pub #field_name: #field_type
                })
            } else {
                None
            }
        });
        tokens.extend(quote! {
            #[derive(Debug, Clone)]
            pub struct #model_name {
                #(#fields,)*
            }
        });
    }
}

impl<'a> AsLocalInsertModel<'a> {
    pub fn get_model_name(&self) -> Ident {
        format_ident!(
            "Insert{}",
            self.0
                .reference_table
                .ident
                .to_string()
                .to_upper_camel_case()
        )
    }
}

struct AsLocalInsertToFull<'a>(&'a SyncGroupTableConfig);

impl<'a> ToTokens for AsLocalInsertToFull<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let local_insert_model_name = AsLocalInsertModel(self.0).get_model_name();
        let full_model_name = AsFullModel(&self.0.reference_table).get_model_name();
        let columns = self
            .0
            .reference_table
            .columns
            .iter()
            .filter_map(|x| {
                match (
                    &x.column_type,
                    &x.client_only_config,
                    &x.mod_on_backend_only_config,
                ) {
                    (&CarburetorColumnType::Data, _, &BackendOnlyConfig::Disabled) => {
                        let field_name = &x.ident;
                        Some(quote! {
                            #field_name: value.#field_name
                        })
                    }
                    (&CarburetorColumnType::Id, _, _) => {
                        let field_name = &x.ident;
                        let prefix = &self.0.reference_table.ident.to_string();
                        Some(quote! {
                            #field_name: carburetor::helpers::generate_id(#prefix.to_string())
                        })
                    }
                    (&CarburetorColumnType::IsDeleted, _, _) => {
                        let field_name = &x.ident;
                        Some(quote! {
                            #field_name: false
                        })
                    }
                    (&CarburetorColumnType::DirtyFlag, _, _) => {
                        let field_name = &x.ident;
                        Some(quote! {
                            #field_name: Some(
                                carburetor::helpers::client_sync_metadata::DirtyFlag::Insert.to_string()
                            )
                        })
                    }
                    (&CarburetorColumnType::ClientColumnSyncMetadata, _, _) => {
                        let field_name = &x.ident;
                        Some(quote! {
                            #field_name: carburetor::serde_json::from_str(
                                &format!(
                                    r#"{{".insert_time": "{}"}}"#,
                                    carburetor::helpers::get_utc_now().to_rfc3339()
                                )
                            ).unwrap()
                        })
                    }
                    (_, ClientOnlyConfig::Enabled { default_value }, _) => {
                        let field_name = &x.ident;
                        Some(quote! {
                            #field_name: #default_value
                        })
                    }
                    (_, _, BackendOnlyConfig::BySqlUtcNow) => {
                        let field_name = &x.ident;
                        Some(quote! {
                            #field_name: None
                        })
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        tokens.extend(quote! {
            impl From<#local_insert_model_name> for #full_model_name {
                fn from(value: #local_insert_model_name) -> Self {
                    Self {
                        #(#columns,)*
                    }
                }
            }
        });
    }
}

pub struct AsLocalUpdateModel<'a>(pub &'a SyncGroupTableConfig);

impl<'a> ToTokens for AsLocalUpdateModel<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = self.get_model_name();
        let fields = self.0.reference_table.columns.iter().filter_map(|x| {
            if x.column_type == CarburetorColumnType::Id {
                let field_name = &x.ident;
                let field_type = AsModelType(&x.diesel_type);
                Some(quote! {
                    pub #field_name: #field_type
                })
            } else if x.column_type == CarburetorColumnType::Data
                && x.mod_on_backend_only_config == BackendOnlyConfig::Disabled
            {
                let field_name = &x.ident;
                let field_type = AsModelType(&x.diesel_type);
                Some(quote! {
                    pub #field_name: Option<#field_type>
                })
            } else {
                None
            }
        });
        tokens.extend(quote! {
            #[derive(Debug, Clone)]
            pub struct #model_name {
                #(#fields,)*
            }
        });
    }
}

impl<'a> AsLocalUpdateModel<'a> {
    pub fn get_model_name(&self) -> Ident {
        format_ident!(
            "Update{}",
            self.0
                .reference_table
                .ident
                .to_string()
                .to_upper_camel_case()
        )
    }
}

struct AsLocalUpdateToChangeset<'a>(&'a SyncGroupTableConfig);

impl<'a> ToTokens for AsLocalUpdateToChangeset<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let local_update_model_name = AsLocalUpdateModel(self.0).get_model_name();
        let changeset_model_name = AsChangesetModel(&self.0.reference_table).get_model_name();
        let columns = self
            .0
            .reference_table
            .columns
            .iter()
            .filter_map(|x| {
                match (
                    &x.column_type,
                    &x.client_only_config,
                    &x.mod_on_backend_only_config,
                ) {
                    (&CarburetorColumnType::Id, _, _)
                    | (&CarburetorColumnType::Data, _, &BackendOnlyConfig::Disabled) => {
                        let field_name = &x.ident;
                        Some(quote! {
                            #field_name: value.#field_name
                        })
                    }
                    _ => {
                        let field_name = &x.ident;
                        Some(quote! {
                            #field_name: None
                        })
                    }
                }
            })
            .collect::<Vec<_>>();
        tokens.extend(quote! {
            impl From<#local_update_model_name> for #changeset_model_name {
                fn from(value: #local_update_model_name) -> Self {
                    Self {
                        #(#columns,)*
                    }
                }
            }
        });
    }
}

pub fn generate_local_operation_models(tokens: &mut TokenStream, sync_group: &CarburetorSyncGroup) {
    sync_group.table_configs.iter().for_each(|x| {
        tokens.extend(AsLocalInsertModel(x).to_token_stream());
        tokens.extend(AsLocalInsertToFull(x).to_token_stream());
        tokens.extend(AsLocalUpdateModel(x).to_token_stream());
        tokens.extend(AsLocalUpdateToChangeset(x).to_token_stream());
    })
}
