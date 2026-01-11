use std::any::TypeId;

use carburetor::{helpers::get_utc_now, models::DownloadTableResponse, prelude::*};

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
    check_full_models();
    check_insert_models();
    check_update_models();
}

fn check_diesel_tables() {
    let _ = users::table;
    let _ = games::table;
    let _ = policies::table;
}

fn check_insert_models() {
    let _ = TypeId::of::<InsertUser>();
    let _ = TypeId::of::<InsertGame>();
    let _ = InsertPolicy {
        id: "id".to_string(),
        user_id: "user1".to_string(),
        game_id: "game1".to_string(),
        role: "spectator".to_string(),
        is_deleted: false,
    };
}

fn check_full_models() {
    let _ = TypeId::of::<FullUser>();
    let _ = TypeId::of::<FullGame>();
    let _ = FullPolicy {
        id: "id".to_string(),
        user_id: "user1".to_string(),
        game_id: "game1".to_string(),
        role: "spectator".to_string(),
        last_synced_at: get_utc_now(),
        is_deleted: false,
    };
}

fn check_update_models() {
    let _ = TypeId::of::<ChangesetUser>();
    let _ = TypeId::of::<ChangesetGame>();
    let _ = ChangesetPolicy {
        id: "id".to_string(),
        user_id: None,
        game_id: None,
        role: None,
        last_synced_at: None,
        is_deleted: None,
    };
}

fn check_generated_sync_groups() {
    check_download_functions();
    check_download_models();
    check_upload_models();
    check_upload_functions();
}

fn check_download_functions() {
    let _ = user_only::process_download_request;
    let _ = all::process_download_request;
}

fn check_download_models() {
    let _ = TypeId::of::<user_only::DownloadRequest>();
    let _ = TypeId::of::<user_only::DownloadUpdateUser>();
    let _ = TypeId::of::<user_only::DownloadResponse>();

    let _ = all::DownloadRequest {
        user_offset: Some(get_utc_now()),
        game_offset: Some(get_utc_now()),
    };
    let _ = all::DownloadUpdateUser {
        user_id: "user".to_string(),
        email: "user@example.com".to_string(),
        login_at: get_utc_now(),
        is_deleted: false,
    };
    let _ = TypeId::of::<all::DownloadUpdateGame>();
    let _ = all::DownloadResponse {
        user: DownloadTableResponse {
            cutoff_at: get_utc_now(),
            data: vec![],
        },
        game: DownloadTableResponse {
            cutoff_at: get_utc_now(),
            data: vec![],
        },
    };
}

fn check_upload_models() {
    let _ = TypeId::of::<user_only::UploadRequest>();
    let _ = TypeId::of::<user_only::UploadRequestUser>();
    let _ = TypeId::of::<user_only::UploadInsertUser>();
    let _ = TypeId::of::<user_only::UploadUpdateUser>();
    let _ = TypeId::of::<user_only::UploadResponse>();
    let _ = all::UploadRequest {
        user: vec![],
        game: vec![],
    };
    let _ = TypeId::of::<all::UploadRequestUser>();
    let _ = TypeId::of::<all::UploadInsertUser>();
    let _ = TypeId::of::<all::UploadUpdateUser>();
    let _ = TypeId::of::<all::UploadRequestGame>();
    let _ = TypeId::of::<all::UploadResponse>();
    let _ = all::UploadResponse {
        user: vec![],
        game: vec![],
    };
    let insert_game = all::UploadInsertGame {
        id: "dummy".to_string(),
        play_on: get_utc_now().date_naive(),
        score: None,
        is_deleted: false,
    };
    let update_game = all::UploadUpdateGame {
        id: "dummy".to_string(),
        play_on: None,
        score: None,
        is_deleted: None,
    };
    let _ = InsertGame::from(insert_game);
    let _ = ChangesetGame::from(update_game);
}

fn check_upload_functions() {
    let _ = user_only::process_upload_request;
    let _ = all::process_upload_request;
}

fn main() {
    check_generated_tables();
    check_generated_sync_groups();
}
