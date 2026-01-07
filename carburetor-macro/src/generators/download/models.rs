use std::{cell::RefCell, rc::Rc};

use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Type, parse_str};

use crate::parsers::{sync_group::CarburetorSyncGroup, table::CarburetorTable};

struct AsRequestField<'a>(&'a Rc<RefCell<CarburetorTable>>);

impl<'a> ToTokens for AsRequestField<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let field_name =
            parse_str::<Type>(&format!("{}_offset", self.0.borrow().ident.to_string())).unwrap();
        tokens.extend(quote! {
            pub #field_name: Option<carburetor::chrono::DateTimeUtc>
        });
    }
}

struct AsResponseField<'a>(&'a Rc<RefCell<CarburetorTable>>);

impl<'a> ToTokens for AsResponseField<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let field_name;
        let model_name;
        {
            let table = self.0.borrow();
            field_name = table.ident.clone();
            model_name = parse_str::<Type>(&table.ident.to_string().to_upper_camel_case()).unwrap();
        }
        tokens.extend(quote! {
            pub #field_name: carburetor::backend::models::DownloadSyncResponse<#model_name>
        });
    }
}

pub(crate) fn generate_download_sync_group_models(
    tokens: &mut TokenStream,
    sync_group: &CarburetorSyncGroup,
) {
    let request_model_name = parse_str::<Type>(&format!(
        "Download{}Request",
        sync_group.name.to_string().to_upper_camel_case()
    ))
    .unwrap();
    let response_model_name = parse_str::<Type>(&format!(
        "Download{}Response",
        sync_group.name.to_string().to_upper_camel_case()
    ))
    .unwrap();

    let request_fields = sync_group
        .tables
        .iter()
        .map(|x| AsRequestField(x))
        .collect::<Vec<_>>();
    let response_fields = sync_group
        .tables
        .iter()
        .map(|x| AsResponseField(x))
        .collect::<Vec<_>>();

    tokens.extend(quote! {
        #[derive(Debug, Clone, Default)]
        pub struct #request_model_name {
            #(#request_fields,)*
        }

        #[derive(Debug, Clone)]
        pub struct #response_model_name {
            #(#response_fields,)*
        }
    });
}
