use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Ident, Type, parse_str};

use crate::parsers::sync_group::CarburetorSyncGroup;

#[derive(Debug, Clone)]
pub(crate) struct AsSyncContext<'a>(pub(crate) &'a CarburetorSyncGroup);

impl<'a> AsSyncContext<'a> {
    pub(crate) fn get_model_name(&self) -> Ident {
        Ident::new("SyncContext", self.0.name.span())
    }

    pub(crate) fn has_context(&self) -> bool {
        !self.0.contexts.is_empty()
    }
}

impl<'a> ToTokens for AsSyncContext<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if !self.has_context() {
            return;
        }
        let model_name = self.get_model_name();
        let fields = self
            .0
            .contexts
            .iter()
            .map(|(var_name, diesel_type)| {
                let field_ident = parse_str::<Ident>(var_name).unwrap();
                let field_type = parse_str::<Type>(&diesel_type.get_model_type_string()).unwrap();
                quote!(pub #field_ident: #field_type)
            })
            .collect::<Vec<_>>();

        tokens.extend(quote! {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            pub struct #model_name {
                #(#fields,)*
            }
        });
    }
}

pub fn generate_context_models(tokens: &mut TokenStream, sync_group: &CarburetorSyncGroup) {
    tokens.extend(AsSyncContext(sync_group).to_token_stream());
}
