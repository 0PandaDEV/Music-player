#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod db;
mod utils;
mod music;

use crate::db::{ music::MusicDatabase, settings::SettingsDatabase };
use sqlx::sqlite::SqlitePoolOptions;
use tauri_plugin_aptabase::{ InitOptions, EventTracker };
use std::fs;
use tauri::Manager;
use tauri_plugin_prevent_default::Flags;
use crate::music::player::AudioPlayer;

#[tokio::main]
async fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    tauri::Builder
        ::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_aptabase::Builder
                ::new("A-SH-4648501883")
                .with_options(InitOptions {
                    host: Some("https://aptabase.pandadev.net".to_string()),
                    flush_interval: None,
                })
                .with_panic_hook(
                    Box::new(|client, info, msg| {
                        let location = info
                            .location()
                            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
                            .unwrap_or_else(|| "".to_string());

                        let _ = client.track_event(
                            "panic",
                            Some(
                                serde_json::json!({
                                    "info": format!("{} ({})", msg, location),
                                })
                            )
                        );
                    })
                )
                .build()
        )
        .plugin(
            tauri_plugin_prevent_default::Builder
                ::new()
                .with_flags(Flags::all().difference(Flags::CONTEXT_MENU))
                .build()
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().unwrap();
            utils::logger::init_logger(&app_data_dir).expect("Failed to initialize logger");

            let _ = app.track_event("app_started", None);

            let db_path = app_data_dir.join("data.db");
            let is_new_db = !db_path.exists();
            if is_new_db {
                fs::File::create(&db_path).expect("Failed to create database file");
            }

            let db_url = format!("sqlite:{}", db_path.to_str().unwrap());

            let app_handle = app.handle().clone();
            let update_handle = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                utils::updater::check_for_updates(update_handle).await;

                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect(&db_url).await
                    .expect("Failed to create pool");

                let music_db = MusicDatabase { pool: pool.clone() };
                let settings_db = SettingsDatabase { pool };

                app_handle.manage(music_db);
                app_handle.manage(settings_db);
            });

            let _ = db::database::setup(app);
            utils::discord_rpc::connect_rpc().ok();

            let (audio_player, _stream) = AudioPlayer::setup();
            app.manage(audio_player);

            Ok(())
        })
        .invoke_handler(
            tauri::generate_handler![
                music::player::set_looping,
                music::player::set_muted,
                music::player::set_volume,
                music::player::set_eq_settings,
                music::player::skip,
                music::player::skip_to,
                music::player::seek,
                music::player::load_song,
                music::player::play,
                music::player::pause,
                music::player::play_pause,
                music::player::rewind,
                db::music::add_playlist,
                db::music::add_song,
                db::music::add_song_to_history,
                db::music::add_song_to_playlist,
                db::music::clear_history,
                db::music::get_history,
                db::music::get_playlist,
                db::music::get_playlists,
                db::music::get_song,
                db::music::get_songs,
                db::music::remove_song,
                db::music::remove_song_from_history,
                db::music::remove_song_from_playlist,
                db::music::remove_playlist,
                db::music::remove_album,
                db::music::add_album,
                db::music::get_album,
                db::settings::get_api_url,
                db::settings::get_current_song,
                db::settings::get_eq,
                db::settings::get_lossless,
                db::settings::get_loop,
                db::settings::get_muted,
                db::settings::get_queue,
                db::settings::get_shuffle,
                db::settings::get_streaming,
                db::settings::get_volume,
                db::settings::set_api_url,
                db::settings::set_current_song,
                db::settings::set_eq,
                db::settings::set_lossless,
                db::settings::set_loop,
                db::settings::set_muted_settings,
                db::settings::set_queue,
                db::settings::set_shuffle,
                db::settings::set_streaming,
                db::settings::set_volume_settings,
                utils::commands::download_from_backend,
                utils::commands::get_music_path,
                utils::commands::ping_urls,
                utils::discord_rpc::clear_activity,
                utils::discord_rpc::update_activity
            ]
        )
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
