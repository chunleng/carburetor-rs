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

    // first_name is nullable → re-adding it succeeds (users table migration
    // passes, first_name is restored)
    diesel::sql_query("ALTER TABLE users DROP COLUMN first_name")
        .execute(&mut conn)
        .unwrap();

    // Change subject type on the messages table to cause a type mismatch.
    // Covers the case where users migrates before messages: users succeeds
    // (first_name re-added), then messages fails — the whole transaction
    // rolls back.
    diesel::sql_query("ALTER TABLE messages ALTER COLUMN subject TYPE BIGINT USING 0")
        .execute(&mut conn)
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("type mismatch"));

    // first_name was re-added by the users migration, but the messages
    // migration failure rolled back the entire transaction; first_name must
    // still be gone
    let columns: Vec<ColumnMeta> = backend
        .test_helper_get_table_columns(ctx(), "users".to_string())
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.name == "first_name")
        .collect();
    assert_eq!(columns.len(), 0);
}

#[tokio::test]
async fn test_partial_migration_rolls_back_all_changes_reversed() {
    // Without this test, the test above passes trivially if messages
    // migrates before users (users never runs, first_name was never
    // re-added, so "first_name still gone" proves nothing about rollback).
    // This reverse scenario guarantees that regardless of migration order,
    // at least one test exercises actual rollback.
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    let db_url = backend.test_helper_get_database_url(ctx()).await.unwrap();
    let mut conn = diesel::PgConnection::establish(&db_url).unwrap();

    // notes is nullable → re-adding it succeeds (messages table migration
    // passes, notes is restored)
    diesel::sql_query("ALTER TABLE messages DROP COLUMN notes")
        .execute(&mut conn)
        .unwrap();

    // Change priority type on the users table to cause a type mismatch.
    // Covers the case where messages migrates before users: messages succeeds
    // (notes re-added), then users fails — the whole transaction rolls back.
    diesel::sql_query("ALTER TABLE users ALTER COLUMN priority TYPE BIGINT USING 0")
        .execute(&mut conn)
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("type mismatch"));

    // notes was re-added by the messages migration, but the users migration
    // failure rolled back the entire transaction; notes must still be gone
    let columns: Vec<ColumnMeta> = backend
        .test_helper_get_table_columns(ctx(), "messages".to_string())
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.name == "notes")
        .collect();
    assert_eq!(columns.len(), 0);
}

#[tokio::test]
async fn test_type_mismatch_fails() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    let db_url = backend.test_helper_get_database_url(ctx()).await.unwrap();
    let mut conn = diesel::PgConnection::establish(&db_url).unwrap();

    // priority is declared as Integer in the schema; change it to bigint
    diesel::sql_query("ALTER TABLE users ALTER COLUMN priority TYPE BIGINT USING 0")
        .execute(&mut conn)
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("type mismatch"));
    assert!(err.contains("priority"));
}

#[tokio::test]
async fn test_primary_key_mismatch_fails() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    let db_url = backend.test_helper_get_database_url(ctx()).await.unwrap();
    let mut conn = diesel::PgConnection::establish(&db_url).unwrap();

    // id is declared as primary key in the schema; drop the PK constraint
    diesel::sql_query("ALTER TABLE users DROP CONSTRAINT users_pkey")
        .execute(&mut conn)
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("primary key mismatch"));
    assert!(err.contains("id"));
}

#[tokio::test]
async fn test_nullable_tightening_fails() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    let db_url = backend.test_helper_get_database_url(ctx()).await.unwrap();
    let mut conn = diesel::PgConnection::establish(&db_url).unwrap();

    // username is declared NOT NULL in the schema; make it nullable
    diesel::sql_query("ALTER TABLE users ALTER COLUMN username DROP NOT NULL")
        .execute(&mut conn)
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("nullability mismatch"));
    assert!(err.contains("username"));
}
