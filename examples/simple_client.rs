use std::time::Duration;

use carburetor::{
    chrono::NaiveDate,
    config::{CarburetorGlobalConfig, initialize_carburetor_global_config},
    helpers::{
        client_sync_metadata::{ClientSyncMetadata, DirtyFlag, Metadata},
        get_utc_now,
    },
    models::{DownloadTableResponse, DownloadTableResponseData, UploadTableResponseData},
};
use diesel::{RunQueryDsl, prelude::*, update};

use crate::schema::all_clients;

#[path = "schema.rs"]
mod schema;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_path =
        std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./default.db".to_string());
    let mut connection =
        SqliteConnection::establish(&database_path).expect("Error connecting to database");
    initialize_carburetor_global_config(CarburetorGlobalConfig { database_path });

    diesel::sql_query("DROP TABLE IF EXISTS users").execute(&mut connection)?;
    diesel::sql_query("DROP TABLE IF EXISTS carburetor_offsets").execute(&mut connection)?;
    diesel::sql_query(
        "CREATE TABLE users(
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL,
            first_name TEXT,
            joined_on DATE NOT NULL,
            last_synced_at TIMESTAMPTZ,
            is_deleted BOOLEAN NOT NULL,
            dirty_flag TEXT,
            column_sync_metadata JSON NOT NULL
        )",
    )
    .execute(&mut connection)?;
    diesel::sql_query(
        "CREATE TABLE carburetor_offsets(
            table_name TEXT PRIMARY KEY,
            cutoff_at TIMESTAMPTZ NOT NULL
        )",
    )
    .execute(&mut connection)?;

    println!("Check download sync offsets (Null for all):");
    dbg!(all_clients::retrieve_download_request()?);

    println!("Insert user_1:");
    schema::all_clients::store_download_response(all_clients::DownloadResponse {
        user: DownloadTableResponse {
            cutoff_at: get_utc_now(),
            data: vec![DownloadTableResponseData::Update(
                all_clients::DownloadUpdateUser {
                    id: "FromBackend".to_string(),
                    username: "user_1".to_string(),
                    first_name: None,
                    joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    last_synced_at: get_utc_now(),
                    is_deleted: false,
                },
            )],
        },
    })?;
    dbg!(
        all_clients::users::table
            .select(all_clients::FullUser::as_select())
            .load::<all_clients::FullUser>(&mut connection)?
    );

    println!("Check download sync offsets (Offset updated):");
    dbg!(all_clients::retrieve_download_request()?);

    println!("Sync FromBackend update from backend:");
    schema::all_clients::store_download_response(all_clients::DownloadResponse {
        user: DownloadTableResponse {
            cutoff_at: get_utc_now(),
            data: vec![DownloadTableResponseData::Update(
                all_clients::DownloadUpdateUser {
                    id: "FromBackend".to_string(),
                    username: "updated_user_1".to_string(),
                    first_name: None,
                    joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    last_synced_at: get_utc_now(),
                    is_deleted: false,
                },
            )],
        },
    })?;
    dbg!(
        all_clients::users::table
            .select(all_clients::FullUser::as_select())
            .load::<all_clients::FullUser>(&mut connection)?
    );

    println!("Unable to update when sync record is older:");
    schema::all_clients::store_download_response(all_clients::DownloadResponse {
        user: DownloadTableResponse {
            cutoff_at: get_utc_now(),
            data: vec![DownloadTableResponseData::Update(
                all_clients::DownloadUpdateUser {
                    id: "FromBackend".to_string(),
                    username: "updated_user_0".to_string(),
                    first_name: None,
                    joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    last_synced_at: get_utc_now() - Duration::from_hours(2),
                    is_deleted: false,
                },
            )],
        },
    })?;
    dbg!(
        all_clients::users::table
            .select(all_clients::FullUser::as_select())
            .load::<all_clients::FullUser>(&mut connection)?
    );

    println!("Skip update to column that is more updated:");
    update(all_clients::users::table.find("FromBackend"))
        .set(all_clients::ChangesetUser {
            id: "FromBackend".to_string(),
            username: None,
            first_name: None,
            joined_on: None,
            last_synced_at: None,
            is_deleted: None,
            dirty_flag: None,
            column_sync_metadata: Some(
                carburetor::helpers::client_sync_metadata::ClientSyncMetadata::<
                    schema::all_clients::UserSyncMetadata,
                > {
                    data: Some(schema::all_clients::UserSyncMetadata {
                        username: Some(carburetor::helpers::client_sync_metadata::Metadata {
                            column_last_synced_at: Some(get_utc_now() + Duration::from_hours(1)),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
                .into(),
            ),
        })
        .execute(&mut connection)?;
    schema::all_clients::store_download_response(all_clients::DownloadResponse {
        user: DownloadTableResponse {
            cutoff_at: get_utc_now(),
            data: vec![DownloadTableResponseData::Update(
                all_clients::DownloadUpdateUser {
                    id: "FromBackend".to_string(),
                    username: "future_record".to_string(), // This is not updated
                    first_name: Some("Marty".to_string()),
                    joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    last_synced_at: get_utc_now(),
                    is_deleted: false,
                },
            )],
        },
    })?;
    dbg!(
        all_clients::users::table
            .select(all_clients::FullUser::as_select())
            .load::<all_clients::FullUser>(&mut connection)?
    );

    println!("Skip update to dirty data records:");
    update(all_clients::users::table.find("FromBackend"))
        .set(all_clients::ChangesetUser {
            id: "FromBackend".to_string(),
            username: None,
            first_name: None,
            joined_on: None,
            last_synced_at: None,
            is_deleted: None,
            dirty_flag: Some(Some(DirtyFlag::Update.to_string())),
            column_sync_metadata: Some(
                ClientSyncMetadata::<schema::all_clients::UserSyncMetadata> {
                    data: Some(schema::all_clients::UserSyncMetadata {
                        username: Some(Metadata {
                            dirty_at: Some(get_utc_now()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
                .into(),
            ),
        })
        .execute(&mut connection)?;
    schema::all_clients::store_download_response(all_clients::DownloadResponse {
        user: DownloadTableResponse {
            cutoff_at: get_utc_now(),
            data: vec![DownloadTableResponseData::Update(
                all_clients::DownloadUpdateUser {
                    id: "FromBackend".to_string(),
                    username: "dirty_record".to_string(), // This is not updated
                    first_name: Some("John".to_string()),
                    joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    last_synced_at: get_utc_now(),
                    is_deleted: false,
                },
            )],
        },
    })?;
    dbg!(
        all_clients::users::table
            .select(all_clients::FullUser::as_select())
            .load::<all_clients::FullUser>(&mut connection)?
    );

    println!("Insert record locally:");
    let inserted_record = dbg!(all_clients::insert_user(all_clients::InsertUser {
        username: "test".to_string(),
        first_name: Some("Jane".to_string()),
        joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    }))?;

    println!("Delete record locally:");
    let _ = dbg!(all_clients::delete_user("FromBackend".to_string()));

    println!("View active records:");
    dbg!(
        all_clients::active_users()
            .select(all_clients::FullUser::as_select())
            .load::<all_clients::FullUser>(&mut connection)?
    );

    println!("Both dirty records are retrieved:");
    let (cutoff_at, _) = dbg!(all_clients::retrieve_upload_request())?;

    // Send records and got back upload_response
    let upload_response = all_clients::UploadResponse {
        user: vec![
            Ok(UploadTableResponseData {
                id: inserted_record.id.clone(),
                last_synced_at: get_utc_now(),
            }),
            Ok(UploadTableResponseData {
                id: "FromBackend".to_string(),
                last_synced_at: get_utc_now(),
            }),
        ],
    };

    println!(
        "Update record happening between upload request and response (To test retain dirtiness):"
    );
    let _ = dbg!(all_clients::update_user(all_clients::UpdateUser {
        id: inserted_record.id,
        first_name: Some(Some("updated_locally".to_string())),
        username: None,
        joined_on: None,
    }));

    let _ = all_clients::store_upload_response(cutoff_at, upload_response);

    dbg!(
        all_clients::users::table
            .select(all_clients::FullUser::as_select())
            .load::<all_clients::FullUser>(&mut connection)?
    );

    Ok(())
}
