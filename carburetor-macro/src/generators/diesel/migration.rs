use std::rc::Rc;

use proc_macro2::TokenStream;
use quote::quote;

use crate::parsers::table::CarburetorTable;
use crate::parsers::table::column::{CarburetorColumnType, ColumnScope};
use crate::parsers::table::postgres_type::DieselPostgresType;

fn column_def(column: &Rc<crate::parsers::table::column::CarburetorColumn>) -> Option<TokenStream> {
    if matches!(column.column_scope, ColumnScope::ClientOnly) {
        return None;
    }

    let name = column.ident.to_string();
    let sql_type = column.diesel_type.get_sql_type_string().to_string();
    let primary_key = matches!(column.column_type, CarburetorColumnType::Id);
    let null = matches!(&column.diesel_type, DieselPostgresType::Generic1(_, _));

    Some(quote! {
        carburetor::helpers::migration::ColumnDef {
            name: #name,
            sql_type: #sql_type,
            primary_key: #primary_key,
            null: #null,
        }
    })
}

pub(crate) fn generate_run_migrations(tokens: &mut TokenStream, tables: &[Rc<CarburetorTable>]) {
    let table_migrations = tables.iter().map(|table| {
        let table_name = &table.plural_ident;
        let table_name_str = table_name.to_string();
        let column_defs: Vec<TokenStream> = table.columns.iter().filter_map(column_def).collect();
        let column_count = column_defs.len();
        quote! {
            {
                let columns: [carburetor::helpers::migration::ColumnDef; #column_count] = [#(#column_defs),*];
                let exists = carburetor::helpers::migration::check_table_exists(conn, #table_name_str)?;
                if !exists {
                    carburetor::helpers::migration::create_table(conn, #table_name_str, &columns)?;
                }
            }
        }
    });

    tokens.extend(quote! {
        pub fn run_migrations(conn: &mut diesel::PgConnection) -> Result<(), carburetor::error::Error> {
            use diesel::Connection;
            conn.transaction(|conn| {
                #(#table_migrations)*
                Ok(())
            }).map_err(|e|
                carburetor::error::Error::Unhandled {
                    message: "Migration error".to_string(),
                    source: e,
                }
            )
        }
    });
}
