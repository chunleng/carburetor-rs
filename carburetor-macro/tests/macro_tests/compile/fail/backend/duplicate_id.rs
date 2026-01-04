use carburetor::prelude::*;

#[carburetor(table_name = "users")]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
}

fn main() {}
