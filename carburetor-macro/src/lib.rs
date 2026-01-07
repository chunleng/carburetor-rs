mod generators;
mod helpers;
mod parsers;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::parse_macro_input;

use crate::{generators::generate_carburetor_sync_config, parsers::CarburetorSyncConfig};

#[proc_macro]
pub fn carburetor_sync_config(input: TokenStream) -> TokenStream {
    let sync_group = parse_macro_input!(input as CarburetorSyncConfig);
    let mut tokens = TokenStream2::new();
    generate_carburetor_sync_config(&mut tokens, sync_group);
    tokens.into()
}
