#[cfg(for_backend)]
pub struct ColumnDef {
    pub name: &'static str,
    pub sql_type: &'static str,
    pub primary_key: bool,
    pub null: bool,
    pub default: Option<String>,
}

#[cfg(for_backend)]
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

#[cfg(for_backend)]
pub fn check_table_exists(
    conn: &mut diesel::PgConnection,
    table_name: &str,
) -> crate::error::Result<bool> {
    use diesel::RunQueryDsl;

    #[derive(diesel::QueryableByName)]
    struct Row {
        #[diesel(sql_type = diesel::sql_types::Bool)]
        exists: bool,
    }

    let result: Row = diesel::sql_query(
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
    )
    .bind::<diesel::sql_types::Text, _>(table_name)
    .get_result(conn)
    .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
        message: format!("Failed to check if table '{}' exists", table_name),
        source: e.into(),
    })?;

    Ok(result.exists)
}

#[cfg(for_backend)]
/// Maps PostgreSQL `information_schema.columns.data_type` values to the
/// uppercase SQL type strings used by carburetor's `ColumnDef`.
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

#[cfg(for_backend)]
pub fn alter_table(
    conn: &mut diesel::PgConnection,
    table_name: &str,
    declared_columns: &[ColumnDef],
) -> crate::error::Result<()> {
    use diesel::RunQueryDsl;

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
    }

    let existing: Vec<Row> = diesel::sql_query(
        "SELECT c.column_name, \
         c.data_type, \
         CASE WHEN c.is_nullable = 'YES' THEN true ELSE false END AS is_nullable, \
         COALESCE(pk.is_primary_key, false) AS is_primary_key \
         FROM information_schema.columns c \
         LEFT JOIN ( \
           SELECT kcu.column_name AS column_name, true AS is_primary_key \
           FROM information_schema.table_constraints tc \
           JOIN information_schema.key_column_usage kcu \
             ON tc.constraint_name = kcu.constraint_name \
             AND tc.table_schema = kcu.table_schema \
           WHERE tc.constraint_type = 'PRIMARY KEY' \
             AND tc.table_name = $1 \
         ) pk ON c.column_name = pk.column_name \
         WHERE c.table_name = $1",
    )
    .bind::<diesel::sql_types::Text, _>(table_name)
    .load(conn)
    .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
        message: format!("Failed to introspect columns of table '{}'", table_name),
        source: e.into(),
    })?;

    for col in declared_columns {
        if let Some(db_col) = existing.iter().find(|e| e.column_name == col.name) {
            let db_type = normalize_pg_data_type(&db_col.data_type);
            if db_type != col.sql_type {
                return Err(crate::error::Error::Migration(format!(
                    "Column '{}' on table '{}' has a type mismatch: \
                     schema declares '{}', but the database has '{}'.",
                    col.name, table_name, col.sql_type, db_type
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

    let missing: Vec<&ColumnDef> = declared_columns
        .iter()
        .filter(|col| !existing.iter().any(|e| e.column_name == col.name))
        .collect();

    for col in &missing {
        if !col.null && col.default.is_none() {
            return Err(crate::error::Error::Migration(format!(
                "Cannot add column '{}' to table '{}': no default specified. \
                 Adding a non-nullable column without a default to a table with existing rows is \
                 not supported.",
                col.name, table_name
            )));
        }
    }

    for col in &missing {
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
    }

    let needs_drop_not_null: Vec<&ColumnDef> = declared_columns
        .iter()
        .filter(|col| {
            col.null
                && existing
                    .iter()
                    .any(|e| e.column_name == col.name && !e.is_nullable)
        })
        .collect();

    for col in &needs_drop_not_null {
        let query = format!(
            "ALTER TABLE {} ALTER COLUMN {} DROP NOT NULL",
            table_name, col.name
        );
        diesel::sql_query(&query)
            .execute(conn)
            .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
                message: format!(
                    "Failed to make column '{}' nullable on table '{}'",
                    col.name, table_name
                ),
                source: e.into(),
            })?;
    }

    Ok(())
}

#[cfg(for_backend)]
pub fn create_table(
    conn: &mut diesel::PgConnection,
    table_name: &str,
    columns: &[ColumnDef],
) -> crate::error::Result<()> {
    use diesel::RunQueryDsl;

    let column_defs_str = columns
        .iter()
        .map(|col| col.to_sql())
        .collect::<Vec<String>>()
        .join(", ");

    let query = format!("CREATE TABLE {} ({})", table_name, column_defs_str);

    diesel::sql_query(&query)
        .execute(conn)
        .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
            message: format!("Failed to create table '{}'", table_name),
            source: e.into(),
        })?;

    Ok(())
}
