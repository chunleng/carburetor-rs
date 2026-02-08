pub mod schema {
    use carburetor::prelude::*;

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
}
