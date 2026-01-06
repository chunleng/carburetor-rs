#[cfg(feature = "backend")]
mod backend;

use proc_macro::TokenStream;
use syn::Result;

use crate::CarburetorTable;

pub(crate) fn generate_all(table: CarburetorTable) -> Result<TokenStream> {
    let mut tokens = TokenStream::new();

    #[cfg(feature = "backend")]
    {
        use crate::generators::backend::generate_backend;
        let backend = generate_backend(&table)?;
        tokens.extend(backend);
    }

    Ok(tokens)
}
