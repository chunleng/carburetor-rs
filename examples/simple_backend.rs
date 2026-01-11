use carburetor::{chrono::NaiveDate, config::initialize_carburetor_global_config};
use diesel::{RunQueryDsl, prelude::*};

use crate::schema::all_clients;

#[path = "schema.rs"]
mod schema;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/".to_string());
    let mut connection =
        PgConnection::establish(&database_url).expect("Error connecting to database");
    initialize_carburetor_global_config(carburetor::config::CarburetorGlobalConfig {
        database_url,
    });

    diesel::sql_query("DROP TABLE IF EXISTS users").execute(&mut connection)?;
    diesel::sql_query(
        "CREATE TABLE users(
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL,
            first_name TEXT,
            joined_on DATE,
            last_synced_at TIMESTAMPTZ,
            is_deleted BOOLEAN
        )",
    )
    .execute(&mut connection)?;

    let id = "USER1".to_string();

    let _ = dbg!(all_clients::process_upload_request(
        all_clients::UploadRequest {
            user: vec![
                all_clients::UploadRequestUser::Insert(all_clients::UploadInsertUser {
                    id: id.clone(),
                    username: "example_user123".to_string(),
                    first_name: None,
                    joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    is_deleted: false,
                }),
                all_clients::UploadRequestUser::Insert(all_clients::UploadInsertUser {
                    id: "USER2".to_string(),
                    username: "example_user123".to_string(),
                    first_name: None,
                    joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    is_deleted: false,
                }),
                all_clients::UploadRequestUser::Insert(all_clients::UploadInsertUser {
                    id: "USER3".to_string(),
                    username: "example_user123".to_string(),
                    first_name: None,
                    joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    is_deleted: true,
                }),
            ],
        }
    ));
    println!(
        "Before Update: 2 Users are printed, Deleted users are filtered when clean downloading"
    );
    let res = dbg!(all_clients::process_download_request(None)?);

    // As ChangesetUser is a Changeset, Any None column will be left untouched
    let _ = dbg!(all_clients::process_upload_request(
        all_clients::UploadRequest {
            user: vec![all_clients::UploadRequestUser::Update(
                all_clients::UploadUpdateUser {
                    id: id.clone(),
                    username: None,
                    first_name: Some(Some("John".to_string())),
                    joined_on: None,
                    is_deleted: Some(true),
                },
            )],
        }
    ));

    println!(
        "After Update: Only User 1 has updated and is printed. Deleted record is included so that client can update accordingly"
    );
    let _ = dbg!(all_clients::process_download_request(Some(
        all_clients::DownloadRequest {
            user_offset: Some(res.user.cutoff_at)
        }
    )));
    Ok(())
}
