#[cfg(feature = "backend")]
mod backend;

use proc_macro::TokenStream;
use syn::Result;

use crate::{CarburetorArgs, TableDetail};

pub(crate) fn generate_all(args: CarburetorArgs, table: TableDetail) -> Result<TokenStream> {
    let mut tokens = TokenStream::new();

    // Soothe the compiler when there's no feature
    let _ = &args.table_name;
    let _ = &table.vis;
    let _ = &table.ident;
    let _ = &table.data_columns.get(0).is_some_and(|x| {
        let _ = x.vis;
        let _ = x.ident;
        true
    });
    let _ = &table.sync_metadata_columns.id;
    let _ = &table.sync_metadata_columns.last_sync_at;
    let _ = &mut tokens;

    #[cfg(feature = "backend")]
    {
        use crate::generators::backend::generate_backend;
        let backend = generate_backend(&args, &table)?;
        tokens.extend(backend);
    }

    Ok(tokens)
}
