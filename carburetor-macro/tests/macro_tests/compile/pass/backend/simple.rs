use carburetor::prelude::*;

#[carburetor(table_name = "users")]
pub struct User {
    pub username: String,
    pub email: String,
}

#[carburetor(table_name = "games")]
pub struct Game {
    #[id]
    pub match_id: String,
    pub score: i32,
    pub match_date: carburetor::chrono::NaiveDate,
}

fn main() {
    let _ = users::table;
    let _ = std::any::TypeId::of::<User>();
    let _ = std::any::TypeId::of::<UpdateUser>();
    let _ = games::table;
    let _ = std::any::TypeId::of::<Game>();
    let _ = std::any::TypeId::of::<UpdateGame>();
}
