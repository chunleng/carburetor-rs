use sample_test_core::ColumnMeta;
use e2e_test::TestBackendHandle;
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

    assert_eq!(users_columns, vec![
        ColumnMeta { name: "created_at".into(), is_primary_key: false, is_nullable: false },
        ColumnMeta { name: "first_name".into(), is_primary_key: false, is_nullable: true },
        ColumnMeta { name: "id".into(), is_primary_key: true, is_nullable: false },
        ColumnMeta { name: "is_deleted".into(), is_primary_key: false, is_nullable: false },
        ColumnMeta { name: "joined_on".into(), is_primary_key: false, is_nullable: false },
        ColumnMeta { name: "last_synced_at".into(), is_primary_key: false, is_nullable: false },
        ColumnMeta { name: "username".into(), is_primary_key: false, is_nullable: false },
    ]);

    let mut messages_columns = backend
        .test_helper_get_table_columns(ctx(), "messages".to_string())
        .await
        .unwrap();
    messages_columns.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(messages_columns, vec![
        ColumnMeta { name: "body".into(), is_primary_key: false, is_nullable: false },
        ColumnMeta { name: "id".into(), is_primary_key: true, is_nullable: false },
        ColumnMeta { name: "is_deleted".into(), is_primary_key: false, is_nullable: false },
        ColumnMeta { name: "last_synced_at".into(), is_primary_key: false, is_nullable: false },
        ColumnMeta { name: "recipient_id".into(), is_primary_key: false, is_nullable: false },
        ColumnMeta { name: "subject".into(), is_primary_key: false, is_nullable: false },
    ]);
}
