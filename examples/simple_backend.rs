use std::time::{SystemTime, UNIX_EPOCH};

use carburetor::carburetor;
use diesel::{RunQueryDsl, prelude::*, update};

#[carburetor(table_name = "users")]
pub struct User {
    pub username: String,
    pub first_name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/".to_string());
    let mut connection =
        PgConnection::establish(&database_url).expect("Error connecting to database");

    diesel::sql_query("DROP TABLE IF EXISTS users").execute(&mut connection)?;
    diesel::sql_query(
        "CREATE TABLE users(id TEXT PRIMARY KEY, username TEXT NOT NULL, first_name TEXT)",
    )
    .execute(&mut connection)?;

    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();
    User {
        id: id.clone(),
        username: "example_user123".to_string(),
        first_name: None,
    }
    .insert_into(users::table)
    .execute(&mut connection)
    .unwrap();

    println!("Before Update:");
    dbg!(
        users::table
            .select(User::as_select())
            .load(&mut connection)?
    );

    // As UpdateUser is a Changeset, Any None column will be left untouched
    let update_user = UpdateUser {
        id: id.clone(),
        username: None,
        first_name: Some(Some("John".to_string())),
    };
    dbg!(
        update(users::table.find(&update_user.id))
            .set(&update_user)
            .execute(&mut connection)?
    );

    println!("After Update:");
    dbg!(
        users::table
            .select(User::as_select())
            .load(&mut connection)
            .unwrap()
    );
    Ok(())
}
