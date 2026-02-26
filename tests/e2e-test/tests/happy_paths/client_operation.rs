use carburetor::chrono::NaiveDate;
use diesel::{RunQueryDsl, SelectableHelper, query_dsl::methods::SelectDsl};
use e2e_test::get_clean_test_client_db;
use sample_test_core::schema::user_only;

#[tokio::test]
async fn test_insert_user() {
    let mut conn = get_clean_test_client_db().get_connection();

    // Insert a user using the generated client function
    let inserted_user = user_only::insert_user(user_only::InsertUser {
        username: "test_username".to_string(),
        first_name: Some("John".to_string()),
        joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
    })
    .unwrap();

    // Verify the user was inserted correctly
    assert_eq!(inserted_user.username, "test_username");
    assert_eq!(inserted_user.first_name, Some("John".to_string()));
    assert_eq!(
        inserted_user.joined_on,
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
    );
    assert_eq!(inserted_user.is_deleted, false);
    assert!(inserted_user.id.starts_with("user-"));
    assert!(inserted_user.dirty_flag.is_some());
    assert_eq!(inserted_user.dirty_flag.as_ref().unwrap(), "insert");

    // Verify the user exists in the database
    let stored_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].id, inserted_user.id);
    assert_eq!(stored_users[0].username, "test_username");

    // Now update the user and check that dirty_flag remains "insert"
    let updated_user = user_only::update_user(user_only::UpdateUser {
        username: Some("updated_username".to_string()),
        first_name: Some(Some("Jane".to_string())),
        joined_on: None,
        id: inserted_user.id.clone(),
    })
    .unwrap();
    assert_eq!(updated_user.dirty_flag.as_deref(), Some("insert"));
    assert_eq!(updated_user.username, "updated_username");
    assert_eq!(updated_user.first_name, Some("Jane".to_string()));
    assert_eq!(
        updated_user.joined_on,
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
    );

    // Check that column_sync_metadata has dirty_at for updated fields
    let meta = &updated_user.column_sync_metadata;
    let username_dirty = meta.get("username").and_then(|v| v.get("dirty_at"));
    let first_name_dirty = meta.get("first_name").and_then(|v| v.get("dirty_at"));
    let joined_on_dirty = meta.get("joined_on").and_then(|v| v.get("dirty_at"));
    assert!(
        username_dirty.is_some(),
        "username.dirty_at should be present"
    );
    assert!(
        first_name_dirty.is_some(),
        "first_name.dirty_at should be present"
    );
    assert!(
        joined_on_dirty.is_none(),
        "joined_on.dirty_at should not be present"
    );
}

#[tokio::test]
async fn test_active_users() {
    let mut conn = get_clean_test_client_db().get_connection();

    // Insert two users: one active, one soft-deleted
    let active_user = user_only::FullUser {
        username: "active_user".to_string(),
        first_name: Some("Alice".to_string()),
        joined_on: NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
        id: "user-active-1".to_string(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: None,
        column_sync_metadata: carburetor::serde_json::from_str("{}").unwrap(),
    };
    let deleted_user = user_only::FullUser {
        username: "deleted_user".to_string(),
        first_name: Some("Bob".to_string()),
        joined_on: NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
        id: "user-deleted-1".to_string(),
        last_synced_at: None,
        is_deleted: true,
        dirty_flag: None,
        column_sync_metadata: carburetor::serde_json::from_str("{}").unwrap(),
    };
    diesel::insert_into(user_only::users::table)
        .values(&active_user)
        .execute(&mut conn)
        .unwrap();
    diesel::insert_into(user_only::users::table)
        .values(&deleted_user)
        .execute(&mut conn)
        .unwrap();

    // Query active users
    let active_users: Vec<user_only::FullUser> = user_only::active_users()
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(active_users.len(), 1);
    assert_eq!(active_users[0].id, active_user.id);
    assert_eq!(active_users[0].username, "active_user");
}

#[tokio::test]
async fn test_delete_user() {
    let mut conn = get_clean_test_client_db().get_connection();

    // First, insert a user directly using diesel
    let test_user = user_only::FullUser {
        username: "test_username".to_string(),
        first_name: Some("John".to_string()),
        joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
        id: "user-test-123".to_string(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: None,
        column_sync_metadata: carburetor::serde_json::from_str("{}").unwrap(),
    };

    diesel::insert_into(user_only::users::table)
        .values(&test_user)
        .execute(&mut conn)
        .unwrap();

    // Delete the user using the generated client function
    let deleted_user = user_only::delete_user(test_user.id.clone()).unwrap();

    // Verify the user was marked as deleted
    assert_eq!(deleted_user.id, test_user.id);
    assert_eq!(deleted_user.username, "test_username");
    assert_eq!(deleted_user.first_name, Some("John".to_string()));
    assert_eq!(deleted_user.is_deleted, true);
    assert!(deleted_user.dirty_flag.is_some());
    assert_eq!(deleted_user.dirty_flag.as_ref().unwrap(), "update");

    // Verify the user is marked as deleted in the database
    let stored_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].id, test_user.id);
    assert_eq!(stored_users[0].is_deleted, true);
}

#[tokio::test]
async fn test_update_user() {
    let mut conn = get_clean_test_client_db().get_connection();

    // First, insert a user directly using diesel
    let test_user = user_only::FullUser {
        username: "original_username".to_string(),
        first_name: Some("John".to_string()),
        joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
        id: "user-test-456".to_string(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: None,
        column_sync_metadata: carburetor::serde_json::from_str("{}").unwrap(),
    };

    diesel::insert_into(user_only::users::table)
        .values(&test_user)
        .execute(&mut conn)
        .unwrap();

    // Update the user using the generated client function
    let updated_user = user_only::update_user(user_only::UpdateUser {
        username: Some("updated_username".to_string()),
        first_name: Some(Some("Jane".to_string())),
        joined_on: Some(NaiveDate::from_ymd_opt(2025, 2, 1).unwrap()),
        id: test_user.id.clone(),
    })
    .unwrap();

    // Verify the user was updated correctly
    assert_eq!(updated_user.id, test_user.id);
    assert_eq!(updated_user.username, "updated_username");
    assert_eq!(updated_user.first_name, Some("Jane".to_string()));
    assert_eq!(
        updated_user.joined_on,
        NaiveDate::from_ymd_opt(2025, 2, 1).unwrap()
    );
    assert_eq!(updated_user.is_deleted, false);
    assert!(updated_user.dirty_flag.is_some());
    assert_eq!(updated_user.dirty_flag.as_ref().unwrap(), "update");

    // Verify the user is updated in the database
    let stored_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].id, test_user.id);
    assert_eq!(stored_users[0].username, "updated_username");
    assert_eq!(stored_users[0].first_name, Some("Jane".to_string()));
    assert_eq!(
        stored_users[0].joined_on,
        NaiveDate::from_ymd_opt(2025, 2, 1).unwrap()
    );
    assert_eq!(stored_users[0].is_deleted, false);
    assert_eq!(stored_users[0].dirty_flag.as_ref().unwrap(), "update");
}

