use std::rc::Rc;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use crate::helpers::{TargetType, get_target_type};
use crate::parsers::table::CarburetorTable;
use crate::parsers::table::column::{
    CarburetorColumn, CarburetorColumnType, ColumnScope, DefaultValue, SqlDefault,
};
use crate::parsers::table::postgres_type::DieselPostgresType;

struct AsColumnDef<'a>(&'a Rc<CarburetorColumn>);

impl<'a> ToTokens for AsColumnDef<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let column = &self.0;
        let name = column.ident.to_string();
        let primary_key = matches!(column.column_type, CarburetorColumnType::Id);

        let (sql_type, null) = match get_target_type() {
            TargetType::Backend => (
                column.diesel_type.get_sql_type_string().to_string(),
                matches!(&column.diesel_type, DieselPostgresType::Generic1(_, _)),
            ),
            TargetType::Client => (
                column.diesel_type.get_sqlite_ddl_type_string().to_string(),
                matches!(&column.diesel_type, DieselPostgresType::Generic1(_, _))
                    || matches!(column.column_scope, ColumnScope::ModOnBackendOnly),
            ),
        };

        let default_str = column.default_value.as_ref().and_then(|dv| match dv {
            DefaultValue::Sql(sql_default) => {
                let ddl = match get_target_type() {
                    TargetType::Backend => {
                        sql_default_to_postgres_ddl(sql_default, &column.diesel_type)
                    }
                    TargetType::Client => {
                        sql_default_to_sqlite_ddl(sql_default, &column.diesel_type)
                    }
                };
                Some(ddl)
            }
            DefaultValue::Rust(_) => None,
        });

        let default_tokens = match &default_str {
            Some(s) => quote! { Some(#s.to_string()) },
            None => quote! { None },
        };

        tokens.extend(quote! {
            carburetor::helpers::migration::ColumnDef {
                name: #name,
                sql_type: #sql_type,
                primary_key: #primary_key,
                null: #null,
                default: #default_tokens,
            }
        });
    }
}

fn sql_default_to_postgres_ddl(
    sql_default: &SqlDefault,
    diesel_type: &DieselPostgresType,
) -> String {
    match sql_default {
        SqlDefault::Null => "NULL".to_string(),
        SqlDefault::EmptyJson => "'{}'::jsonb".to_string(),
        SqlDefault::Text(s) => format!("'{}'", s.replace("'", "''")),
        SqlDefault::Number(n) => n.clone(),
        SqlDefault::Now => match diesel_type.unwrap_nullable() {
            DieselPostgresType::Timestamptz | DieselPostgresType::Timestamp => "now()".to_string(),
            DieselPostgresType::Date => "CURRENT_DATE".to_string(),
            DieselPostgresType::Time => "CURRENT_TIME".to_string(),
            _ => unreachable!("type compatibility validated at parse time"),
        },
    }
}

fn sql_default_to_sqlite_ddl(sql_default: &SqlDefault, diesel_type: &DieselPostgresType) -> String {
    match sql_default {
        SqlDefault::Null => "NULL".to_string(),
        SqlDefault::EmptyJson => "'{}'".to_string(),
        SqlDefault::Text(s) => format!("'{}'", s.replace("'", "''")),
        SqlDefault::Number(n) => n.clone(),
        SqlDefault::Now => match diesel_type.unwrap_nullable() {
            DieselPostgresType::Timestamptz | DieselPostgresType::Timestamp => {
                "(datetime('now'))".to_string()
            }
            DieselPostgresType::Date => "(date('now'))".to_string(),
            DieselPostgresType::Time => "(time('now'))".to_string(),
            _ => unreachable!("type compatibility validated at parse time"),
        },
    }
}

pub(crate) fn generate_run_migrations(tokens: &mut TokenStream, tables: &[Rc<CarburetorTable>]) {
    let is_client = get_target_type() == TargetType::Client;

    let conn_type = if is_client {
        quote!(diesel::SqliteConnection)
    } else {
        quote!(diesel::PgConnection)
    };

    let mut table_migrations: Vec<TokenStream> = tables
        .iter()
        .map(|table| {
            let table_name_str = table.plural_ident.to_string();
            let column_defs: Vec<TokenStream> = table
                .columns
                .iter()
                .filter(|c| is_client || !matches!(c.column_scope, ColumnScope::ClientOnly))
                .map(|c| AsColumnDef(c).to_token_stream())
                .collect();
            let column_count = column_defs.len();

            let alter_block = if is_client {
                // TODO: alter_table for client to be added later
                quote! {}
            } else {
                quote! {
                    else {
                        carburetor::helpers::migration::alter_table(conn, #table_name_str, &columns)?;
                    }
                }
            };

            quote! {
                {
                    let columns: [carburetor::helpers::migration::ColumnDef; #column_count] = [#(#column_defs),*];
                    let exists = carburetor::helpers::migration::check_table_exists(conn, #table_name_str)?;
                    if !exists {
                        carburetor::helpers::migration::create_table(conn, #table_name_str, &columns)?;
                    }
                    #alter_block
                }
            }
        })
        .collect();

    if is_client {
        table_migrations.insert(
            0,
            quote! {
                {
                    let columns: [carburetor::helpers::migration::ColumnDef; 2] = [
                        carburetor::helpers::migration::ColumnDef {
                            name: "table_name",
                            sql_type: "TEXT",
                            primary_key: true,
                            null: false,
                            default: None,
                        },
                        carburetor::helpers::migration::ColumnDef {
                            name: "cutoff_at",
                            sql_type: "TIMESTAMPTZ",
                            primary_key: false,
                            null: false,
                            default: None,
                        },
                    ];
                    let exists = carburetor::helpers::migration::check_table_exists(conn, "carburetor_offsets")?;
                    if !exists {
                        carburetor::helpers::migration::create_table(conn, "carburetor_offsets", &columns)?;
                    }
                }
            },
        );
    }

    tokens.extend(quote! {
        pub fn run_migrations(conn: &mut #conn_type) -> Result<(), carburetor::error::Error> {
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

#[cfg(all(test, feature = "migration"))]
mod tests {
    use super::*;
    use crate::parsers::table::column::SqlDefault;
    use crate::parsers::table::postgres_type::DieselPostgresType;

    #[test]
    fn text_default_with_apostrophe_is_escaped() {
        let result = sql_default_to_postgres_ddl(
            &SqlDefault::Text("it's a test".to_string()),
            &DieselPostgresType::Text,
        );
        assert_eq!(result, "'it''s a test'");
    }

    #[test]
    fn text_default_without_apostrophe_unaffected() {
        let result = sql_default_to_postgres_ddl(
            &SqlDefault::Text("no preference".to_string()),
            &DieselPostgresType::Text,
        );
        assert_eq!(result, "'no preference'");
    }
}
