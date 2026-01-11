use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::Ident;

use crate::{
    generators::diesel::models::AsModelType,
    parsers::{
        sync_group::{CarburetorSyncGroup, SyncGroupTableConfig},
        table::column::{BackendOnlyConfig, CarburetorColumnType, ClientOnlyConfig},
    },
};

#[cfg(feature = "client")]
pub mod client {
    use std::ops::Deref;

    use proc_macro2::TokenStream;
    use quote::{ToTokens, format_ident, quote};
    use syn::Ident;

    use super::{AsUploadInsertTable, AsUploadRequestTable, AsUploadUpdateTable};
    use crate::{
        generators::{client::models::AsTableMetadata, diesel::models::AsFullModel},
        parsers::{
            sync_group::SyncGroupTableConfig,
            table::column::{BackendOnlyConfig, CarburetorColumnType, ClientOnlyConfig},
        },
    };

    pub struct AsFromFullToTable<'a>(pub &'a SyncGroupTableConfig);

    impl<'a> AsFromFullToTable<'a> {
        pub fn get_function_name(&self) -> Ident {
            format_ident!("into_upload_request")
        }
    }

    impl<'a> ToTokens for AsFromFullToTable<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let function_name = self.get_function_name();
            let full_model_name = AsFullModel(&self.0.reference_table).get_model_name();
            let upload_request_table_name = AsUploadRequestTable(self.0).get_model_name();
            let upload_insert_table_name = AsUploadInsertTable(self.0).get_model_name();
            let upload_insert_table_fields = self
                .0
                .reference_table
                .columns
                .iter()
                .filter_map(|x| {
                    if x.client_only_config == ClientOnlyConfig::Disabled
                        && x.mod_on_backend_only_config == BackendOnlyConfig::Disabled
                    {
                        let field_name = &x.ident;
                        Some(quote!(#field_name: self.#field_name))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let upload_update_table_name = AsUploadUpdateTable(self.0).get_model_name();
            let upload_update_table_fields = self
                .0
                .reference_table
                .columns
                .iter()
                .filter_map(|x| {
                    if x.column_type == CarburetorColumnType::Id {
                        let field_name = &x.ident;
                        Some(quote!(#field_name: self.#field_name))
                    } else if x.client_only_config == ClientOnlyConfig::Disabled
                        && x.mod_on_backend_only_config == BackendOnlyConfig::Disabled
                    {
                        let field_name = &x.ident;
                        Some(quote! {
                            #field_name: match sync_metadata.#field_name {
                                Some(carburetor::helpers::client_sync_metadata::Metadata {
                                    dirty_at: Some(dirty_at), ..
                                }) if dirty_at <= cutoff_time => Some(self.#field_name),
                                _ => None
                            }
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            let dirty_flag_column = &self
                .0
                .reference_table
                .sync_metadata_columns
                .dirty_flag
                .deref()
                .ident;
            let client_column_metadata_column = &self
                .0
                .reference_table
                .sync_metadata_columns
                .client_column_sync_metadata
                .deref()
                .ident;

            let sync_metadata_model_name =
                AsTableMetadata(&self.0.reference_table).get_struct_name();

            tokens.extend(quote! {
            impl #full_model_name {
                fn #function_name(self, cutoff_time: carburetor::chrono::DateTimeUtc) -> Option<#upload_request_table_name> {
                    use carburetor::helpers::client_sync_metadata::DirtyFlag;
                    match self.#dirty_flag_column {
                        Some(ref x) if x == &DirtyFlag::Insert.to_string() => {
                            Some(#upload_request_table_name::Insert(#upload_insert_table_name {
                                #(#upload_insert_table_fields,)*
                            }))
                        }
                        Some(ref x) if x == &DirtyFlag::Update.to_string() => {
                            let sync_metadata: #sync_metadata_model_name = carburetor::serde_json::from_value(self.#client_column_metadata_column).unwrap_or_default();
                            Some(#upload_request_table_name::Update(#upload_update_table_name {
                                #(#upload_update_table_fields,)*
                            }))
                        }
                        _ => None,
                    }
                }
            }
        });
        }
    }
}

#[cfg(feature = "backend")]
pub mod backend {
    use proc_macro2::TokenStream;
    use quote::{ToTokens, quote};

    use super::{AsUploadInsertTable, AsUploadUpdateTable};
    use crate::{
        generators::diesel::models::{AsChangesetModel, backend::AsInsertModel},
        parsers::{
            sync_group::SyncGroupTableConfig,
            table::column::{BackendOnlyConfig, ClientOnlyConfig},
        },
    };

    pub struct AsFromUploadInsertToInsertModel<'a>(pub &'a SyncGroupTableConfig);

    impl<'a> ToTokens for AsFromUploadInsertToInsertModel<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let upload_insert_model_name = AsUploadInsertTable(self.0).get_model_name();
            let insert_model_name = AsInsertModel(&self.0.reference_table).get_model_name();

            let columns = self
                .0
                .reference_table
                .columns
                .iter()
                .filter_map(|x| {
                    if x.client_only_config != ClientOnlyConfig::Disabled {
                        return None;
                    }

                    let field_name = &x.ident;

                    match x.mod_on_backend_only_config {
                        BackendOnlyConfig::Disabled => Some(quote!(#field_name: value.#field_name)),
                        BackendOnlyConfig::BySqlUtcNow => None,
                    }
                })
                .collect::<Vec<_>>();

            tokens.extend(quote! {
                impl From<#upload_insert_model_name> for super::#insert_model_name {
                    fn from(value: #upload_insert_model_name) -> Self {
                        Self {
                            #(#columns,)*
                        }
                    }
                }
            });
        }
    }

    pub struct AsFromUploadUpdateToChangeset<'a>(pub &'a SyncGroupTableConfig);

    impl<'a> ToTokens for AsFromUploadUpdateToChangeset<'a> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let upload_update_model_name = AsUploadUpdateTable(self.0).get_model_name();
            let changeset_model_name = AsChangesetModel(&self.0.reference_table).get_model_name();

            let columns = self
                .0
                .reference_table
                .columns
                .iter()
                .filter_map(|x| {
                    if x.client_only_config != ClientOnlyConfig::Disabled {
                        return None;
                    }

                    let field_name = &x.ident;

                    match x.mod_on_backend_only_config {
                        BackendOnlyConfig::BySqlUtcNow => Some(quote!(#field_name: None)),
                        BackendOnlyConfig::Disabled => Some(quote!(#field_name: value.#field_name)),
                    }
                })
                .collect::<Vec<_>>();

            tokens.extend(quote! {
                impl From<#upload_update_model_name> for super::#changeset_model_name {
                    fn from(value: #upload_update_model_name) -> Self {
                        Self {
                            #(#columns,)*
                        }
                    }
                }
            });
        }
    }
}

struct AsUploadUpdateTable<'a>(&'a SyncGroupTableConfig);

impl<'a> ToTokens for AsUploadUpdateTable<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = self.get_model_name();
        let fields = self.0.reference_table.columns.iter().filter_map(|x| {
            if x.column_type == CarburetorColumnType::Id {
                let field_name = &x.ident;
                let field_type = AsModelType(&x.diesel_type);
                Some(quote! {
                    pub #field_name: #field_type
                })
            } else if x.client_only_config == ClientOnlyConfig::Disabled
                && x.mod_on_backend_only_config == BackendOnlyConfig::Disabled
            {
                let field_name = &x.ident;
                let field_type = AsModelType(&x.diesel_type);
                Some(quote! {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    pub #field_name: Option<#field_type>
                })
            } else {
                None
            }
        });
        tokens.extend(quote! {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            pub struct #model_name {
                #(#fields,)*
            }
        });
    }
}

impl<'a> AsUploadUpdateTable<'a> {
    fn get_model_name(&self) -> Ident {
        format_ident!(
            "UploadUpdate{}",
            self.0
                .reference_table
                .ident
                .to_string()
                .to_upper_camel_case()
        )
    }
}

struct AsUploadInsertTable<'a>(&'a SyncGroupTableConfig);

impl<'a> ToTokens for AsUploadInsertTable<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = self.get_model_name();
        let fields = self.0.reference_table.columns.iter().filter_map(|x| {
            if x.client_only_config == ClientOnlyConfig::Disabled
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
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            pub struct #model_name {
                #(#fields,)*
            }
        });
    }
}

impl<'a> AsUploadInsertTable<'a> {
    fn get_model_name(&self) -> Ident {
        format_ident!(
            "UploadInsert{}",
            self.0
                .reference_table
                .ident
                .to_string()
                .to_upper_camel_case()
        )
    }
}

pub struct AsUploadRequestTable<'a>(pub &'a SyncGroupTableConfig);

impl<'a> ToTokens for AsUploadRequestTable<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = self.get_model_name();
        let insert_model_name = AsUploadInsertTable(self.0).get_model_name();
        let update_model_name = AsUploadUpdateTable(self.0).get_model_name();
        tokens.extend(quote! {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            pub enum #model_name {
                Insert(#insert_model_name),
                Update(#update_model_name),
            }
        });
    }
}

impl<'a> AsUploadRequestTable<'a> {
    pub fn get_model_name(&self) -> Ident {
        format_ident!(
            "UploadRequest{}",
            self.0
                .reference_table
                .ident
                .to_string()
                .to_upper_camel_case()
        )
    }
}

pub struct AsUploadRequest<'a>(pub &'a CarburetorSyncGroup);

impl<'a> ToTokens for AsUploadRequest<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = self.get_model_name();
        let fields = self.0.table_configs.iter().map(|x| {
            let request_table_model = AsUploadRequestTable(x).get_model_name();
            let field_name = &x.reference_table.ident;
            quote! {
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub #field_name: Vec<#request_table_model>
            }
        });
        tokens.extend(quote! {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            pub struct #model_name {
                #(#fields,)*
            }
        });
    }
}

impl<'a> AsUploadRequest<'a> {
    pub fn get_model_name(&self) -> Ident {
        Ident::new("UploadRequest", self.0.name.span())
    }
}

pub struct AsUploadResponseModel<'a>(pub &'a CarburetorSyncGroup);

impl<'a> AsUploadResponseModel<'a> {
    pub fn get_model_name(&self) -> Ident {
        Ident::new("UploadResponse", self.0.name.span())
    }
}

impl<'a> ToTokens for AsUploadResponseModel<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = self.get_model_name();
        let fields = self.0.table_configs.iter().map(|x| {
            let field_name = &x.reference_table.ident;
            quote! {
                #[serde(skip_serializing_if = "Vec::is_empty")]
                pub #field_name: Vec<Result<carburetor::models::UploadTableResponseData, carburetor::models::UploadTableResponseError>>
            }
        });

        tokens.extend(quote! {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            pub struct #model_name {
                #(#fields,)*
            }
        });
    }
}

pub fn generate_upload_sync_group_models(
    tokens: &mut TokenStream,
    sync_group: &CarburetorSyncGroup,
) {
    let upload_request = AsUploadRequest(sync_group);
    let upload_response = AsUploadResponseModel(sync_group);
    let models_from_table = sync_group.table_configs.iter().map(|x| {
        let request_table = AsUploadRequestTable(x);
        let insert_table = AsUploadInsertTable(x);
        let update_table = AsUploadUpdateTable(x);
        let conversion_functions: TokenStream;

        #[cfg(feature = "client")]
        {
            use crate::generators::upload::models::client::AsFromFullToTable;
            let from_full_to_table = AsFromFullToTable(x);
            conversion_functions = quote!(#from_full_to_table);
        }
        #[cfg(feature = "backend")]
        {
            use crate::generators::upload::models::backend::{
                AsFromUploadInsertToInsertModel, AsFromUploadUpdateToChangeset,
            };
            let from_insert_to_full = AsFromUploadInsertToInsertModel(x);
            let from_update_to_changeset = AsFromUploadUpdateToChangeset(x);
            conversion_functions = quote!(#from_insert_to_full #from_update_to_changeset);
        }

        quote! {
            #request_table
            #insert_table
            #update_table
            #conversion_functions
        }
    });

    tokens.extend(quote! {
        #upload_request
        #upload_response
        #(#models_from_table)*
    });
}
