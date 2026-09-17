mod config;
mod desktop;
mod discord;
mod models;
mod playback;
mod plugins;
mod remote;
mod servers;
mod updater;
mod youtube;

use std::sync::Arc;

use tauri::Manager;

use models::AppState;

pub fn run() {
    let state = Arc::new(AppState::default());
    let setup_state = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .setup(move |app| {
            match servers::start_stream_server(app.handle().clone()) {
                Ok(base_url) => {
                    if let Ok(mut target) = setup_state.stream_base_url.lock() {
                        *target = Some(base_url);
                    }
                }
                Err(error) => eprintln!("Could not start Yolite stream server: {error}"),
            }
            match servers::start_controller_server(app.handle().clone(), setup_state.clone()) {
                Ok(url) => {
                    if let Ok(mut target) = setup_state.controller_url.lock() {
                        *target = Some(url);
                    }
                }
                Err(error) => eprintln!("Could not start Yolite phone controller: {error}"),
            }
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(icon) =
                    tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                {
                    let _ = window.set_icon(icon);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            youtube::get_session,
            youtube::save_session,
            youtube::clear_session,
            youtube::import_browser_session,
            youtube::open_youtube_music,
            youtube::open_app_login,
            youtube::import_app_login_session,
            playback::play_track_native,
            playback::stop_native_playback,
            playback::get_native_playback,
            playback::get_native_visualizer,
            playback::set_native_volume,
            playback::set_native_equalizer,
            playback::set_native_visualizer,
            playback::set_native_pause,
            playback::seek_native_playback,
            youtube::search_tracks,
            youtube::get_home,
            youtube::get_library,
            youtube::get_playlist,
            youtube::get_mix,
            playback::prefetch_track,
            youtube::set_track_liked,
            youtube::create_playlist,
            youtube::add_track_to_playlist,
            playback::resolve_track,
            discord::update_discord_presence,
            discord::clear_discord_presence,
            desktop::set_global_shortcuts,
            desktop::set_mini_player,
            plugins::load_plugins,
            remote::update_remote_state,
            remote::get_remote_controller,
            updater::check_for_update,
            updater::install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running Yolite");
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::json;

    use crate::{config::*, models::*, playback::*, servers::*, youtube::*};

    #[test]
    fn test_valid_video_id() {
        assert!(valid_video_id("dQw4w9WgXcQ"));
        assert!(valid_video_id("abcdef"));
        assert!(!valid_video_id("abc"));
        assert!(!valid_video_id("abc;rm -rf"));
    }

    #[test]
    fn test_normalize_playlist_id() {
        assert_eq!(
            normalize_playlist_id("VLPL123_abc-def").unwrap(),
            "PL123_abc-def"
        );
        assert_eq!(normalize_playlist_id("LM").unwrap(), "LM");
        assert!(normalize_playlist_id("").is_err());
        assert!(normalize_playlist_id("PL123;rm").is_err());
    }

    #[test]
    fn test_validate_channel_id_rejects_user_id() {
        assert_eq!(
            validate_channel_id("https://www.youtube.com/channel/UC12345678901234567890").unwrap(),
            "UC12345678901234567890"
        );
        assert!(validate_channel_id("BYCrdMbiZRv2jBUKbJCDHw").is_err());
    }

    #[test]
    fn test_extract_user_ids_from_account_page_reads_brand_urls() {
        let ids = extract_user_ids_from_account_page(
            r#"href=\"\/b\/115266920129326433551\/settings\" href="%2Fb%2F115266920129326433551%2F""#,
        );

        assert_eq!(ids, vec!["115266920129326433551"]);
    }

    #[test]
    fn test_playlist_hint_score_matches_expected_titles() {
        let playlists = vec![
            PlaylistPayload {
                id: "PL1".to_string(),
                title: "SPACE KEES".to_string(),
                thumbnail: String::new(),
                url: String::new(),
            },
            PlaylistPayload {
                id: "PL2".to_string(),
                title: "Other".to_string(),
                thumbnail: String::new(),
                url: String::new(),
            },
            PlaylistPayload {
                id: "PL3".to_string(),
                title: "jordy playlist".to_string(),
                thumbnail: String::new(),
                url: String::new(),
            },
        ];

        assert_eq!(playlist_hint_score(&playlists), 2);
    }

    #[test]
    fn test_parse_home_two_row_track_item() {
        let item = json!({
            "title": { "runs": [{ "text": "Heavy track" }] },
            "subtitle": {
                "runs": [{
                    "text": "Artist",
                    "navigationEndpoint": {
                        "browseEndpoint": {
                            "browseId": "UCartist",
                            "browseEndpointContextSupportedConfigs": {
                                "browseEndpointContextMusicConfig": {
                                    "pageType": "MUSIC_PAGE_TYPE_ARTIST"
                                }
                            }
                        }
                    }
                }]
            },
            "navigationEndpoint": {
                "watchEndpoint": {
                    "videoId": "abcdef12345",
                    "playlistId": "RDAMVMabcdef12345",
                    "params": "wAEB",
                    "watchEndpointMusicSupportedConfigs": {
                        "watchEndpointMusicConfig": {
                            "musicVideoType": "MUSIC_VIDEO_TYPE_ATV"
                        }
                    }
                }
            }
        });

        let track = parse_two_row_track_item(&item).unwrap();
        assert_eq!(track.id, "abcdef12345");
        assert_eq!(track.kind, "SONG");
        assert_eq!(track.title, "Heavy track");
        assert_eq!(track.artist, "Artist");
        assert_eq!(track.playlist_id, "RDAMVMabcdef12345");
        assert_eq!(track.playlist_params, "wAEB");
    }

    #[test]
    fn test_parse_playlist_panel_track_item() {
        let item = json!({
            "title": { "runs": [{ "text": "Queued track" }] },
            "longBylineText": { "runs": [{
                "text": "Queued artist",
                "navigationEndpoint": {
                    "browseEndpoint": {
                        "browseId": "UCartist",
                        "browseEndpointContextSupportedConfigs": {
                            "browseEndpointContextMusicConfig": {
                                "pageType": "MUSIC_PAGE_TYPE_ARTIST"
                            }
                        }
                    }
                }
            }] },
            "lengthText": { "runs": [{ "text": "3:21" }] },
            "navigationEndpoint": {
                "watchEndpoint": {
                    "videoId": "queued12345",
                    "playlistId": "RDAMVMqueued12345"
                }
            }
        });

        let track = parse_playlist_panel_track_item(&item).unwrap();
        assert_eq!(track.id, "queued12345");
        assert_eq!(track.title, "Queued track");
        assert_eq!(track.artist, "Queued artist");
        assert_eq!(track.duration, 201);
        assert_eq!(track.playlist_id, "RDAMVMqueued12345");
    }

    #[test]
    fn test_equalizer_filter_clamps_and_formats_mpv_filter() {
        let equalizer = EqualizerPayload {
            preamp: 6.0,
            bass: 20.0,
            low_mid: -2.0,
            mid: 0.0,
            high_mid: 3.0,
            treble: -20.0,
            normalization: false,
        };
        let filter = equalizer.mpv_filter().unwrap();

        assert!(filter.starts_with("lavfi=[bass=g=12.0"));
        assert!(filter.contains("equalizer=f=250:t=q:w=1:g=-2.0"));
        assert!(filter.contains("treble=g=-12.0:f=10000"));
        assert!(filter.contains("volume=1.9953"));
        assert!(EqualizerPayload::default().mpv_filter().is_none());
        let normalized = EqualizerPayload {
            normalization: true,
            ..EqualizerPayload::default()
        };
        assert!(normalized.mpv_filter().unwrap().contains("dynaudnorm"));
    }

    #[test]
    fn test_native_visualizer_filter_only_runs_when_enabled() {
        let disabled = native_audio_filters(&EqualizerPayload::default(), false);
        let enabled = native_audio_filters(&EqualizerPayload::default(), true);

        assert!(disabled.is_empty());
        assert!(enabled.contains("@yolite_visualizer"));
        assert!(enabled.contains("aspectralstats"));
    }

    #[test]
    fn test_native_player_keeps_startup_errors_visible() {
        let args = player_args(
            Path::new("/tmp/yolite-test.sock"),
            0.8,
            "Test track",
            "https://stream.example/audio.m4a",
            Path::new("/usr/bin/yt-dlp"),
            &EqualizerPayload::default(),
            false,
        );

        assert!(!args.iter().any(|argument| argument == "--really-quiet"));
        assert!(args.iter().any(|argument| argument == "--no-terminal"));
    }

    #[test]
    fn test_native_visualizer_uses_audio_metadata() {
        let payload = visualizer_payload(&json!({
            "lavfi.astats.Overall.RMS_level": "-18.0",
            "lavfi.astats.Overall.Peak_level": "-8.0",
            "lavfi.aspectralstats.1.centroid": "2400.0",
            "lavfi.aspectralstats.1.flux": "0.03"
        }));

        assert!(payload.energy > 0.5);
        assert!(payload.peak > payload.energy);
        assert!(payload.bass > payload.treble);
        assert!(payload.mids > 0.5);
    }

    #[test]
    fn test_controller_uses_private_token_and_safe_download_name() {
        let token = secure_token().unwrap();
        assert_eq!(token.len(), 36);
        assert!(token.chars().all(|character| character.is_ascii_hexdigit()));
        assert!(controller_html(&token).contains(&format!("const token=\"{token}\"")));
        let name = safe_download_name("Song / ../../ bad?");
        assert!(!name.contains('/'));
        assert!(!name.contains('?'));
    }

    #[test]
    fn test_native_playback_url_uses_cached_stream() {
        let state = AppState::default();
        remember_stream_url(&state, "abcdef12345", "https://stream.example/audio.m4a");
        let resolver_calls = AtomicU64::new(0);

        let url = native_playback_url(&state, "abcdef12345", || {
            resolver_calls.fetch_add(1, Ordering::Relaxed);
            Err("resolver should not run".to_string())
        })
        .unwrap();

        assert_eq!(url, "https://stream.example/audio.m4a");
        assert_eq!(resolver_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_native_playback_url_caches_resolved_stream() {
        let state = AppState::default();

        let url = native_playback_url(&state, "abcdef12345", || {
            Ok("https://stream.example/fresh.m4a".to_string())
        })
        .unwrap();

        assert_eq!(url, "https://stream.example/fresh.m4a");
        assert_eq!(
            cached_stream_url(&state, "abcdef12345"),
            Some("https://stream.example/fresh.m4a".to_string())
        );
    }

    #[test]
    fn test_external_command_clears_appimage_runtime_env() {
        let command = external_command(Path::new("yt-dlp"));
        let envs = command
            .get_envs()
            .map(|(key, value)| (key.to_string_lossy().to_string(), value.is_none()))
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(envs.get("LD_LIBRARY_PATH"), Some(&true));
        assert_eq!(envs.get("LD_PRELOAD"), Some(&true));
        assert_eq!(envs.get("PYTHONHOME"), Some(&true));
        assert_eq!(envs.get("PYTHONPATH"), Some(&true));
    }

    #[test]
    fn test_public_channel_playlists_skip_non_playlist_tiles() {
        let data = json!({
            "musicCarouselShelfRenderer": {
                "header": {
                    "musicCarouselShelfBasicHeaderRenderer": {
                        "title": {
                            "runs": [{
                                "text": "Playlists",
                                "navigationEndpoint": {
                                    "browseEndpoint": {
                                        "browseId": "UC12345678901234567890",
                                        "params": "playlist-params"
                                    }
                                }
                            }]
                        }
                    }
                },
                "contents": [
                    {
                        "musicTwoRowItemRenderer": {
                            "title": { "runs": [{ "text": "Public playlist" }] },
                            "navigationEndpoint": {
                                "browseEndpoint": { "browseId": "VLPL1234567890" }
                            }
                        }
                    },
                    {
                        "musicTwoRowItemRenderer": {
                            "title": { "runs": [{ "text": "Album tile" }] },
                            "navigationEndpoint": {
                                "browseEndpoint": { "browseId": "MPREb_abc" }
                            }
                        }
                    }
                ]
            }
        });

        let playlists = parse_public_channel_playlists(&data);
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].id, "PL1234567890");
        assert_eq!(
            public_channel_playlist_params(&data),
            vec!["playlist-params"]
        );
    }

    #[test]
    fn test_find_candidate_ytdlp() {
        let binary = find_candidate_ytdlp(None);
        assert!(
            binary.is_some(),
            "Expected yt-dlp binary to be found in project"
        );
        let path = binary.unwrap();
        assert!(path.exists(), "Binary path {:?} must exist", path);
        assert!(is_executable(&path), "Binary {:?} must be executable", path);
    }

    #[test]
    fn test_cookie_import_probe_uses_supported_youtube_url() {
        assert!(MUSIC_COOKIE_IMPORT_PROBE_URL.starts_with("https://music.youtube.com/"));
    }

    #[test]
    fn test_netscape_cookie_export_keeps_google_and_youtube_domains() {
        let cookies = normalize_cookies(
            ".youtube.com\tTRUE\t/\tTRUE\t0\tSID\tyoutube-session\n\
             .google.com\tTRUE\t/\tTRUE\t0\tSID\tgoogle-session\n\
             music.youtube.com\tFALSE\t/\tTRUE\t0\tLOGIN_INFO\tlogin-info",
        );

        assert_eq!(cookies, "SID=google-session; LOGIN_INFO=login-info");
    }

    #[test]
    fn test_oversized_cookie_header_keeps_likely_youtube_names() {
        let cookies = normalize_cookies(&format!(
            "unrelated={}; SID=session; __Secure-3PSID=secure-session",
            "x".repeat(MAX_COOKIE_HEADER_BYTES)
        ));

        assert_eq!(cookies, "SID=session; __Secure-3PSID=secure-session");
    }

    #[test]
    fn test_best_thumbnail_reads_nested_thumbnail_arrays() {
        let data = json!({
            "thumbnail": {
                "musicThumbnailRenderer": {
                    "thumbnail": {
                        "thumbnails": [
                            { "url": "https://img.example/small.jpg", "width": 60 },
                            { "url": "https://img.example/large.jpg", "width": 544 }
                        ]
                    }
                }
            }
        });

        assert_eq!(best_thumbnail(&data), "https://img.example/large.jpg");
    }
}
