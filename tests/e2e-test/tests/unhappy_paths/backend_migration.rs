use diesel::Connection;
use diesel::RunQueryDsl;
use e2e_test::TestBackendHandle;
use sample_test_core::ColumnMeta;
use tarpc::context::current as ctx;

#[tokio::test]
async fn test_add_missing_column_without_default_fails() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    diesel::sql_query("ALTER TABLE users DROP COLUMN username")
        .execute(
            &mut diesel::PgConnection::establish(
                &backend.test_helper_get_database_url(ctx()).await.unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no default"));
}

#[tokio::test]
async fn test_partial_migration_rolls_back_all_changes() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    let db_url = backend.test_helper_get_database_url(ctx()).await.unwrap();
    let mut conn = diesel::PgConnection::establish(&db_url).unwrap();

    // first_name is nullable → re-adding it succeeds (add-missing loop passes)
    diesel::sql_query("ALTER TABLE users DROP COLUMN first_name")
        .execute(&mut conn)
        .unwrap();

    // nickname is declared nullable in the schema; make it NOT NULL and the
    // primary key so that ALTER COLUMN ... DROP NOT NULL fails (PK columns
    // cannot have NOT NULL dropped in PostgreSQL)
    diesel::sql_query("ALTER TABLE users DROP CONSTRAINT users_pkey")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query("ALTER TABLE users ALTER COLUMN nickname SET NOT NULL")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query("ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY (nickname)")
        .execute(&mut conn)
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to make column"));

    // first_name was re-added by the add-missing loop, but the DROP NOT NULL
    // failure rolled back the entire transaction; first_name must still be gone
    let columns: Vec<ColumnMeta> = backend
        .test_helper_get_table_columns(ctx(), "users".to_string())
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.name == "first_name")
        .collect();
    assert_eq!(columns.len(), 0);
}
