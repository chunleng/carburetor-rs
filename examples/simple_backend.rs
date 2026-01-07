use carburetor::{chrono::NaiveDate, config::initialize_carburetor_global_config, prelude::*};
use chrono::Utc;
use diesel::{RunQueryDsl, prelude::*, update};

carburetor_sync_config! {
    tables {
        user {
            username -> Text,
            first_name -> Nullable<Text>,
            joined_on -> Date,
        }
    }
    sync_groups {
        all_clients {
            user
        }
    }
}

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
            last_synced_at TIMESTAMPTZ
        )",
    )
    .execute(&mut connection)?;

    let id = "USER1".to_string();
    User {
        id: id.clone(),
        username: "example_user123".to_string(),
        first_name: None,
        joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        last_synced_at: Utc::now(),
    }
    .insert_into(users::table)
    .execute(&mut connection)
    .unwrap();
    User {
        id: "USER2".to_string(),
        username: "example_user123".to_string(),
        first_name: None,
        joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        last_synced_at: Utc::now(),
    }
    .insert_into(users::table)
    .execute(&mut connection)
    .unwrap();
    println!("Before Update: Both Users are printed");
    let res = dbg!(download_all_clients(DownloadAllClientsRequest::default())?);

    // As UpdateUser is a Changeset, Any None column will be left untouched
    let update_user = UpdateUser {
        id: id.clone(),
        username: None,
        first_name: Some(Some("John".to_string())),
        joined_on: None,
        last_synced_at: Some(Utc::now()),
    };
    dbg!(
        update(users::table.find(&update_user.id))
            .set(&update_user)
            .execute(&mut connection)?
    );

    println!("After Update: Only User 1 has update and is printed");
    let _ = dbg!(download_all_clients(DownloadAllClientsRequest {
        user_offset: Some(res.user.last_synced_at)
    }));
    Ok(())
}
