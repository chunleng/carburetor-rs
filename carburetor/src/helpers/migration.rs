pub struct ColumnDef {
    pub name: &'static str,
    pub sql_type: &'static str,
    pub primary_key: bool,
    pub null: bool,
    pub default: Option<String>,
}

impl ColumnDef {
    pub fn to_sql(&self) -> String {
        let mut def = format!("{} {}", self.name, self.sql_type);
        if self.primary_key {
            def.push_str(" PRIMARY KEY");
        }
        if !self.null {
            def.push_str(" NOT NULL");
        }
        if let Some(ref default) = self.default {
            def.push_str(&format!(" DEFAULT {}", default));
        }
        def
    }
}

/// Normalized representation of an existing column in the database, produced
/// by backend-specific introspection.
pub struct ExistingColumn {
    pub name: String,
    pub sql_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
    pub column_default: Option<String>,
}

/// Connection type for `alter_table`, cfg-gated to the active backend.
#[cfg(for_backend)]
pub type MigrationConn = diesel::PgConnection;

#[cfg(for_client)]
pub type MigrationConn = diesel::SqliteConnection;

#[cfg(for_backend)]
pub mod backend {
    use super::{ColumnDef, ExistingColumn};
    use diesel::RunQueryDsl;

    /// Maps PostgreSQL `information_schema.columns.data_type` values to the
    /// uppercase SQL type strings used by carburetor's `ColumnDef`.
    ///
    /// This whitelist must stay in sync with
    /// `DieselPostgresType::get_sql_type_string` in
    /// `carburetor-macro/src/parsers/table/postgres_type.rs`. When a new type
    /// is added there, add the corresponding PG name → SQL string mapping here.
    fn normalize_pg_data_type(data_type: &str) -> &str {
        match data_type {
            "text" => "TEXT",
            "smallint" => "SMALLINT",
            "integer" => "INTEGER",
            "bigint" => "BIGINT",
            "real" => "REAL",
            "double precision" => "DOUBLE PRECISION",
            "boolean" => "BOOLEAN",
            "timestamp without time zone" => "TIMESTAMP",
            "timestamp with time zone" => "TIMESTAMPTZ",
            "date" => "DATE",
            "time without time zone" => "TIME",
            "jsonb" => "JSONB",
            _ => data_type,
        }
    }

    pub fn check_table_exists(
        conn: &mut diesel::PgConnection,
        table_name: &str,
    ) -> crate::error::Result<bool> {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Bool)]
            exists: bool,
        }

        let result: Row = diesel::sql_query(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1 AND table_schema = current_schema())",
        )
        .bind::<diesel::sql_types::Text, _>(table_name)
        .get_result(conn)
        .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
            message: format!("Failed to check if table '{}' exists", table_name),
            source: e.into(),
        })?;

        Ok(result.exists)
    }

    pub(crate) fn introspect_columns(
        conn: &mut diesel::PgConnection,
        table_name: &str,
    ) -> crate::error::Result<Vec<ExistingColumn>> {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Text)]
            column_name: String,
            #[diesel(sql_type = diesel::sql_types::Text)]
            data_type: String,
            #[diesel(sql_type = diesel::sql_types::Bool)]
            is_nullable: bool,
            #[diesel(sql_type = diesel::sql_types::Bool)]
            is_primary_key: bool,
            #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
            column_default: Option<String>,
        }

        let rows: Vec<Row> = diesel::sql_query(
            "SELECT c.column_name, \
             c.data_type, \
             CASE WHEN c.is_nullable = 'YES' THEN true ELSE false END AS is_nullable, \
             COALESCE(pk.is_primary_key, false) AS is_primary_key, \
             c.column_default \
             FROM information_schema.columns c \
             LEFT JOIN ( \
               SELECT kcu.column_name AS column_name, true AS is_primary_key \
               FROM information_schema.table_constraints tc \
               JOIN information_schema.key_column_usage kcu \
                 ON tc.constraint_name = kcu.constraint_name \
                 AND tc.table_schema = kcu.table_schema \
               WHERE tc.constraint_type = 'PRIMARY KEY' \
                 AND tc.table_name = $1 \
                 AND tc.table_schema = current_schema() \
             ) pk ON c.column_name = pk.column_name \
             WHERE c.table_name = $1 \
               AND c.table_schema = current_schema()",
        )
        .bind::<diesel::sql_types::Text, _>(table_name)
        .load(conn)
        .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
            message: format!("Failed to introspect columns of table '{}'", table_name),
            source: e.into(),
        })?;

        Ok(rows
            .into_iter()
            .map(|r| ExistingColumn {
                name: r.column_name,
                sql_type: normalize_pg_data_type(&r.data_type).to_string(),
                is_nullable: r.is_nullable,
                is_primary_key: r.is_primary_key,
                column_default: r.column_default,
            })
            .collect())
    }

    /// Validate that existing columns match the declared schema (type, PK,
    /// nullability).
    pub(crate) fn validate_existing_columns(
        existing: &[ExistingColumn],
        declared: &[ColumnDef],
        table_name: &str,
    ) -> crate::error::Result<()> {
        for col in declared {
            if let Some(db_col) = existing.iter().find(|e| e.name == col.name) {
                if db_col.sql_type != col.sql_type {
                    return Err(crate::error::Error::Migration(format!(
                        "Column '{}' on table '{}' has a type mismatch: \
                         schema declares '{}', but the database has '{}'.",
                        col.name, table_name, col.sql_type, db_col.sql_type
                    )));
                }

                if col.primary_key != db_col.is_primary_key {
                    return Err(crate::error::Error::Migration(format!(
                        "Column '{}' on table '{}' has a primary key mismatch: \
                         schema declares {}, but the database has {}.",
                        col.name, table_name, col.primary_key, db_col.is_primary_key
                    )));
                }

                if !col.null && db_col.is_nullable {
                    return Err(crate::error::Error::Migration(format!(
                        "Column '{}' on table '{}' has a nullability mismatch: \
                         schema declares NOT NULL, but the database allows NULL.",
                        col.name, table_name
                    )));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_extra_columns(
        existing: &[ExistingColumn],
        declared: &[ColumnDef],
        table_name: &str,
    ) -> crate::error::Result<()> {
        for col in existing {
            if !declared.iter().any(|d| d.name == col.name)
                && !col.is_nullable
                && col.column_default.is_none()
            {
                return Err(crate::error::Error::Migration(format!(
                    "Column '{}' on table '{}' is not in the schema, is NOT NULL, and has no default \
                     value. INSERT operations generated by carburetor omit columns not in the schema, \
                     which would violate the NOT NULL constraint. Either make the column nullable, \
                     add a default value, or include it in the schema.",
                    col.name, table_name
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn add_column(
        conn: &mut diesel::PgConnection,
        table_name: &str,
        col: &ColumnDef,
    ) -> crate::error::Result<()> {
        let query = format!(
            "ALTER TABLE {} ADD COLUMN IF NOT EXISTS {}",
            table_name,
            col.to_sql()
        );
        diesel::sql_query(&query)
            .execute(conn)
            .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
                message: format!(
                    "Failed to add column '{}' to table '{}'",
                    col.name, table_name
                ),
                source: e.into(),
            })?;
        Ok(())
    }

    pub(crate) fn drop_not_null(
        conn: &mut diesel::PgConnection,
        table_name: &str,
        col_name: &str,
    ) -> crate::error::Result<()> {
        let query = format!(
            "ALTER TABLE {} ALTER COLUMN {} DROP NOT NULL",
            table_name, col_name
        );
        diesel::sql_query(&query)
            .execute(conn)
            .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
                message: format!(
                    "Failed to make column '{}' nullable on table '{}'",
                    col_name, table_name
                ),
                source: e.into(),
            })?;
        Ok(())
    }
}

#[cfg(for_client)]
pub mod client {
    use super::{ColumnDef, ExistingColumn};
    use diesel::RunQueryDsl;

    pub fn check_table_exists(
        conn: &mut diesel::SqliteConnection,
        table_name: &str,
    ) -> crate::error::Result<bool> {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            count: i64,
        }

        let result: Row = diesel::sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind::<diesel::sql_types::Text, _>(table_name)
        .get_result(conn)
        .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
            message: format!("Failed to check if table '{}' exists", table_name),
            source: e.into(),
        })?;

        Ok(result.count > 0)
    }

    pub(crate) fn introspect_columns(
        conn: &mut diesel::SqliteConnection,
        table_name: &str,
    ) -> crate::error::Result<Vec<ExistingColumn>> {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Text)]
            name: String,
            #[diesel(sql_type = diesel::sql_types::Text)]
            r#type: String,
            #[diesel(sql_type = diesel::sql_types::Integer)]
            notnull: i32,
            #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
            dflt_value: Option<String>,
            #[diesel(sql_type = diesel::sql_types::Integer)]
            pk: i32,
        }

        let rows: Vec<Row> = diesel::sql_query(format!("PRAGMA table_info({})", table_name))
            .load(conn)
            .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
                message: format!("Failed to introspect columns of table '{}'", table_name),
                source: e.into(),
            })?;

        Ok(rows
            .into_iter()
            .map(|r| ExistingColumn {
                name: r.name,
                sql_type: r.r#type,
                is_nullable: r.notnull == 0,
                is_primary_key: r.pk > 0,
                column_default: r.dflt_value,
            })
            .collect())
    }

    pub(crate) fn validate_existing_columns(
        _existing: &[ExistingColumn],
        _declared: &[ColumnDef],
        _table_name: &str,
    ) -> crate::error::Result<()> {
        // TODO: implement affinity-class type-mismatch detection
        Ok(())
    }

    pub(crate) fn validate_extra_columns(
        _existing: &[ExistingColumn],
        _declared: &[ColumnDef],
        _table_name: &str,
    ) -> crate::error::Result<()> {
        // TODO: implement extra-column validation
        Ok(())
    }

    pub(crate) fn add_column(
        conn: &mut diesel::SqliteConnection,
        table_name: &str,
        col: &ColumnDef,
    ) -> crate::error::Result<()> {
        let query = format!("ALTER TABLE {} ADD COLUMN {}", table_name, col.to_sql());
        diesel::sql_query(&query)
            .execute(conn)
            .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
                message: format!(
                    "Failed to add column '{}' to table '{}'",
                    col.name, table_name
                ),
                source: e.into(),
            })?;
        Ok(())
    }

    pub(crate) fn drop_not_null(
        _conn: &mut diesel::SqliteConnection,
        _table_name: &str,
        _col_name: &str,
    ) -> crate::error::Result<()> {
        // TODO: implement DROP NOT NULL via SQLite table-rebuild
        Ok(())
    }
}

#[cfg(for_backend)]
use backend::{
    add_column, drop_not_null, introspect_columns, validate_existing_columns,
    validate_extra_columns,
};

#[cfg(for_client)]
use client::{
    add_column, drop_not_null, introspect_columns, validate_existing_columns,
    validate_extra_columns,
};

pub fn find_missing<'a>(
    existing: &[ExistingColumn],
    declared: &'a [ColumnDef],
) -> Vec<&'a ColumnDef> {
    declared
        .iter()
        .filter(|col| !existing.iter().any(|e| e.name == col.name))
        .collect()
}

pub fn validate_missing_have_defaults(
    missing: &[&ColumnDef],
    table_name: &str,
) -> crate::error::Result<()> {
    for col in missing {
        if !col.null && col.default.is_none() {
            return Err(crate::error::Error::Migration(format!(
                "Cannot add column '{}' to table '{}': no default specified. \
                 Adding a non-nullable column without a default to a table with existing rows is \
                 not supported.",
                col.name, table_name
            )));
        }
    }
    Ok(())
}

pub fn alter_table(
    conn: &mut MigrationConn,
    table_name: &str,
    declared_columns: &[ColumnDef],
) -> crate::error::Result<()> {
    let existing = introspect_columns(conn, table_name)?;

    validate_existing_columns(&existing, declared_columns, table_name)?;
    validate_extra_columns(&existing, declared_columns, table_name)?;

    let missing = find_missing(&existing, declared_columns);
    validate_missing_have_defaults(&missing, table_name)?;

    for col in &missing {
        add_column(conn, table_name, col)?;
    }

    let needs_drop_not_null: Vec<&ColumnDef> = declared_columns
        .iter()
        .filter(|col| {
            col.null
                && existing
                    .iter()
                    .any(|e| e.name == col.name && !e.is_nullable)
        })
        .collect();

    for col in &needs_drop_not_null {
        drop_not_null(conn, table_name, col.name)?;
    }

    Ok(())
}

#[cfg(for_backend)]
pub use backend::check_table_exists;

#[cfg(for_client)]
pub use client::check_table_exists;

pub fn create_table(
    conn: &mut impl diesel::connection::SimpleConnection,
    table_name: &str,
    columns: &[ColumnDef],
) -> crate::error::Result<()> {
    let column_defs_str = columns
        .iter()
        .map(|col| col.to_sql())
        .collect::<Vec<String>>()
        .join(", ");

    let query = format!("CREATE TABLE {} ({})", table_name, column_defs_str);

    conn.batch_execute(&query)
        .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
            message: format!("Failed to create table '{}'", table_name),
            source: e.into(),
        })?;

    Ok(())
}
