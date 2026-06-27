#[cfg(for_backend)]
pub struct ColumnDef {
    pub name: &'static str,
    pub sql_type: &'static str,
    pub primary_key: bool,
    pub null: bool,
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
