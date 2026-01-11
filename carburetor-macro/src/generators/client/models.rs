use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::Ident;

use crate::parsers::{
    sync_group::CarburetorSyncGroup,
    table::{
        CarburetorTable,
        column::{CarburetorColumn, CarburetorColumnType, ClientOnlyConfig},
    },
};

struct AsTableMetadataField<'a>(&'a CarburetorColumn);

impl<'a> ToTokens for AsTableMetadataField<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = &self.0.ident;
        tokens.extend(quote! {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub #ident: Option<carburetor::helpers::client_sync_metadata::Metadata>
        });
    }
}

pub struct AsTableMetadata<'a>(pub &'a CarburetorTable);

impl<'a> ToTokens for AsTableMetadata<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let struct_name = self.get_struct_name();
        let fields = self
            .0
            .columns
            .iter()
            .filter_map(|x| {
                if x.client_only_config == ClientOnlyConfig::Disabled
                    && (CarburetorColumnType::Data == x.column_type
                        || CarburetorColumnType::IsDeleted == x.column_type)
                {
                    // TableMetadata keeps track of local changes so that it can properly update
                    // the backend. This means that we are only interested on columns that are
                    // synced to the backend eventually. This also exclude most metadata columns as
                    // they are involved to ensure the accuracy of the client column sync metadata
                    Some(AsTableMetadataField(x))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        tokens.extend(quote! {
            #[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
            pub struct #struct_name {
                #(#fields,)*
            }
        });
    }
}

impl<'a> AsTableMetadata<'a> {
    pub fn get_struct_name(&self) -> Ident {
        let table_ident = &self.0.ident;
        Ident::new(
            &format!(
                "{}SyncMetadata",
                table_ident.to_string().to_upper_camel_case()
            ),
            table_ident.span(),
        )
    }
}

pub(crate) fn generate_client_models(tokens: &mut TokenStream, sync_group: &CarburetorSyncGroup) {
    let table_metadata = sync_group
        .table_configs
        .iter()
        .map(|x| AsTableMetadata(&x.reference_table))
        .collect::<Vec<_>>();
    tokens.extend(quote! {
        #(#table_metadata)*
    })
}
