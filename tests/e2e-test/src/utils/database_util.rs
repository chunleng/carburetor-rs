use diesel::SqliteConnection;
use diesel::{
    RunQueryDsl, SelectableHelper,
    query_dsl::methods::{FindDsl, SelectDsl},
};
use sample_test_core::schema::user_only;

pub fn retrieve_stored_user(conn: &mut SqliteConnection, id: &str) -> user_only::FullUser {
    user_only::users::table
        .select(user_only::FullUser::as_select())
        .find(id)
        .first(conn)
        .unwrap()
}
