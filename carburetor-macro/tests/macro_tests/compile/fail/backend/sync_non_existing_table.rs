use carburetor::prelude::*;

carburetor_sync_config! {
    tables {
        user {
            email -> Text,
        }
    }
    sync_groups {
        error_group {
            user, non_existent
        }
    }
}

carburetor_sync_config! {
    tables {
        user {
            email -> Text,
        }
    }
    sync_groups {
        error_group2 {
            non_existent
        }
    }
}
fn main() {}
