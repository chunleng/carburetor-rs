use std::any::TypeId;

use carburetor::{
    backend::{helpers::get_utc_now, models::DownloadSyncResponse},
    prelude::*,
};

carburetor_sync_config! {
    tables {
        user() {
            #[id]
            user_id -> Text,
            email -> Text,
            #[last_synced_at]
            login_at -> Timestamptz,
        }
        game {
            play_on -> Date,
            score -> Nullable<Text>,
        }
        policy(plural = "policies") {
            user_id -> Text,
            game_id -> Text,
            role -> Text,
        }
    }
    sync_groups {
        user_only {
            user
        }
        all {
            user, game
        }
    }
}

carburetor_sync_config! {
    sync_groups {}
    tables {}
}

fn check_generated_tables() {
    check_diesel_tables();
    check_query_models();
    check_update_models();
}

fn check_diesel_tables() {
    let _ = users::table;
    let _ = games::table;
    let _ = policies::table;
}

fn check_query_models() {
    let _ = TypeId::of::<User>();
    let _ = TypeId::of::<Game>();
    let _ = Policy {
        id: "id".to_string(),
        user_id: "user1".to_string(),
        game_id: "game1".to_string(),
        role: "spectator".to_string(),
        last_synced_at: carburetor::backend::helpers::get_utc_now(),
    };
}

fn check_update_models() {
    let _ = TypeId::of::<UpdateUser>();
    let _ = TypeId::of::<UpdateGame>();
    let _ = UpdatePolicy {
        id: "id".to_string(),
        user_id: None,
        game_id: None,
        role: None,
        last_synced_at: None,
    };
}

fn check_generated_sync_groups() {
    check_sync_group_functions();
    check_sync_group_models();
}

fn check_sync_group_functions() {
    let _ = download_user_only;
    let _ = download_all;
}

fn check_sync_group_models() {
    let _ = TypeId::of::<DownloadUserOnlyRequest>();
    let _ = TypeId::of::<DownloadUserOnlyResponse>();
    let _ = DownloadAllRequest {
        user_offset: Some(get_utc_now()),
        game_offset: Some(get_utc_now()),
    };
    let _ = DownloadAllResponse {
        user: DownloadSyncResponse {
            last_synced_at: get_utc_now(),
            data: vec![],
        },
        game: DownloadSyncResponse {
            last_synced_at: get_utc_now(),
            data: vec![],
        },
    };
}

fn main() {
    check_generated_tables();
    check_generated_sync_groups();
}
