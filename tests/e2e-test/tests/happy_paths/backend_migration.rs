use diesel::Connection;
use diesel::RunQueryDsl;
use e2e_test::TestBackendHandle;
use sample_test_core::ColumnMeta;
use tarpc::context::current as ctx;

#[tokio::test]
async fn test_clean_migration_of_tables() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    let mut users_columns = backend
        .test_helper_get_table_columns(ctx(), "users".to_string())
        .await
        .unwrap();
    users_columns.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(
        users_columns,
        vec![
            ColumnMeta {
                name: "created_at".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "first_name".into(),
                is_primary_key: false,
                is_nullable: true,
                column_default: None,
            },
            ColumnMeta {
                name: "id".into(),
                is_primary_key: true,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "is_deleted".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "joined_on".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "last_synced_at".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "nickname".into(),
                is_primary_key: false,
                is_nullable: true,
                column_default: None,
            },
            ColumnMeta {
                name: "preferences".into(),
                is_primary_key: false,
                is_nullable: true,
                column_default: Some("'no preference'::text".into()),
            },
            ColumnMeta {
                name: "priority".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: Some("0".into()),
            },
            ColumnMeta {
                name: "username".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
        ]
    );

    let mut messages_columns = backend
        .test_helper_get_table_columns(ctx(), "messages".to_string())
        .await
        .unwrap();
    messages_columns.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(
        messages_columns,
        vec![
            ColumnMeta {
                name: "body".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "id".into(),
                is_primary_key: true,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "is_deleted".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "last_synced_at".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "recipient_id".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
            ColumnMeta {
                name: "subject".into(),
                is_primary_key: false,
                is_nullable: false,
                column_default: None,
            },
        ]
    );
}

#[tokio::test]
async fn test_add_missing_column_with_sql_default() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    diesel::sql_query("ALTER TABLE users DROP COLUMN priority")
        .execute(
            &mut diesel::PgConnection::establish(
                &backend.test_helper_get_database_url(ctx()).await.unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_ok());

    let columns: Vec<ColumnMeta> = backend
        .test_helper_get_table_columns(ctx(), "users".to_string())
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.name == "priority")
        .collect();
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].column_default, Some("0".into()));
}

#[tokio::test]
async fn test_add_nullable_column_without_default() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    diesel::sql_query("ALTER TABLE users DROP COLUMN first_name")
        .execute(
            &mut diesel::PgConnection::establish(
                &backend.test_helper_get_database_url(ctx()).await.unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_ok());

    let columns: Vec<ColumnMeta> = backend
        .test_helper_get_table_columns(ctx(), "users".to_string())
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.name == "first_name")
        .collect();
    assert_eq!(columns.len(), 1);
    assert!(columns[0].is_nullable);
    assert_eq!(columns[0].column_default, None);
}

#[tokio::test]
async fn test_make_existing_column_nullable() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    diesel::sql_query("ALTER TABLE users ALTER COLUMN first_name SET NOT NULL")
        .execute(
            &mut diesel::PgConnection::establish(
                &backend.test_helper_get_database_url(ctx()).await.unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    let result = backend.test_helper_rerun_migrations(ctx()).await.unwrap();
    assert!(result.is_ok());

    let columns: Vec<ColumnMeta> = backend
        .test_helper_get_table_columns(ctx(), "users".to_string())
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.name == "first_name")
        .collect();
    assert_eq!(columns.len(), 1);
    assert!(columns[0].is_nullable);
}
