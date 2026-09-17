use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tauri::Manager;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use crate::{config::*, models::*, youtube::valid_video_id};

pub(crate) fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            let mode = metadata.permissions().mode();
            if mode & 0o111 != 0 {
                return true;
            }
            let mut perms = metadata.permissions();
            perms.set_mode(mode | 0o755);
            let _ = fs::set_permissions(path, perms);
            if let Ok(new_meta) = path.metadata() {
                return new_meta.permissions().mode() & 0o111 != 0;
            }
        }
        false
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub(crate) fn find_candidate_ytdlp(app: Option<&tauri::AppHandle>) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("YOLITE_YTDLP") {
        let p = PathBuf::from(path);
        if is_executable(&p) {
            return Some(p);
        }
    }

    let mut candidates = Vec::new();

    // 1. Tauri resource directory
    if let Some(app) = app {
        if let Ok(resource_dir) = app.path().resource_dir() {
            candidates.push(resource_dir.join("bin/yt-dlp"));
            candidates.push(resource_dir.join("yt-dlp"));
            candidates.push(resource_dir.join("_up_/node_modules/youtube-dl-exec/bin/yt-dlp"));
            candidates.push(resource_dir.join("node_modules/youtube-dl-exec/bin/yt-dlp"));
        }
    }

    // 2. Executable directory and its ancestors
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("bin/yt-dlp"));
            candidates.push(exe_dir.join("yt-dlp"));
            candidates.push(exe_dir.join("_up_/node_modules/youtube-dl-exec/bin/yt-dlp"));
            candidates.push(exe_dir.join("node_modules/youtube-dl-exec/bin/yt-dlp"));

            let mut current = exe_dir.parent();
            while let Some(dir) = current {
                candidates.push(dir.join("node_modules/youtube-dl-exec/bin/yt-dlp"));
                candidates.push(dir.join("bin/yt-dlp"));
                candidates.push(dir.join("yt-dlp"));
                current = dir.parent();
            }
        }
    }

    // 3. Working directory and its ancestors
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("node_modules/youtube-dl-exec/bin/yt-dlp"));
        candidates.push(cwd.join("bin/yt-dlp"));
        candidates.push(cwd.join("yt-dlp"));

        let mut current = cwd.parent();
        while let Some(dir) = current {
            candidates.push(dir.join("node_modules/youtube-dl-exec/bin/yt-dlp"));
            candidates.push(dir.join("bin/yt-dlp"));
            candidates.push(dir.join("yt-dlp"));
            current = dir.parent();
        }
    }

    // 4. System PATH directories
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            candidates.push(dir.join("yt-dlp"));
            #[cfg(windows)]
            candidates.push(dir.join("yt-dlp.exe"));
        }
    }

    // 5. Common user and system installation directories
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local/bin/yt-dlp"));
        candidates.push(home.join(".cargo/bin/yt-dlp"));
    }
    candidates.push(PathBuf::from("/usr/local/bin/yt-dlp"));
    candidates.push(PathBuf::from("/usr/bin/yt-dlp"));

    candidates.into_iter().find(|path| is_executable(path))
}

pub(crate) fn find_ytdlp_binary(app: &tauri::AppHandle) -> Option<PathBuf> {
    find_candidate_ytdlp(Some(app))
}

pub(crate) fn ytdlp_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    find_ytdlp_binary(app).ok_or_else(|| {
        "yt-dlp executable could not be found. Please ensure yt-dlp is installed or run `npm install` to download the bundled binary.".to_string()
    })
}

pub(crate) fn find_mpv_binary() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path_var| {
        std::env::split_paths(&path_var)
            .map(|dir| dir.join("mpv"))
            .find(|path| is_executable(path))
    })
}

pub(crate) fn external_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.env_remove("PYTHONHOME").env_remove("PYTHONPATH");
    command
}

pub(crate) fn stop_playback(state: &AppState) {
    if let Ok(mut playback) = state.playback.lock() {
        if let Some(mut playback) = playback.take() {
            let _ = playback.player.kill();
            let _ = playback.player.wait();
            let _ = fs::remove_file(&playback.ipc_path);
        }
    }
}

pub(crate) fn fade_between_players(
    mut old: NativePlayback,
    new_ipc_path: PathBuf,
    target_volume: f64,
    fade_in: f64,
) {
    thread::spawn(move || {
        let steps = ((fade_in.clamp(0.5, 10.0) * 10.0).round() as u64).max(1);
        let target = target_volume.clamp(0.0, 1.0) * 100.0;
        for step in 0..=steps {
            let ratio = step as f64 / steps as f64;
            let _ = mpv_ipc_command(
                &old.ipc_path,
                json!(["set_property", "volume", target * (1.0 - ratio)]),
            );
            let _ = mpv_ipc_command(
                &new_ipc_path,
                json!(["set_property", "volume", target * ratio]),
            );
            thread::sleep(Duration::from_millis(100));
        }
        let _ = old.player.kill();
        let _ = old.player.wait();
        let _ = fs::remove_file(&old.ipc_path);
    });
}

pub(crate) fn ytdlp_stream_args(video_id: &str, cookie_file: Option<&Path>) -> Vec<String> {
    let mut args = vec![
        "--format".to_string(),
        YTDLP_AUDIO_FORMAT.to_string(),
        "--no-playlist".to_string(),
        "--no-warnings".to_string(),
        "--quiet".to_string(),
        "--output".to_string(),
        "-".to_string(),
        "--add-header".to_string(),
        "referer:https://music.youtube.com".to_string(),
        "--add-header".to_string(),
        "user-agent:Mozilla/5.0".to_string(),
    ];
    if let Some(cookie_file) = cookie_file {
        args.push("--cookies".to_string());
        args.push(cookie_file.display().to_string());
    }
    args.push("--".to_string());
    args.push(format!("https://www.youtube.com/watch?v={video_id}"));
    args
}

pub(crate) fn player_args(
    ipc_path: &Path,
    volume: f64,
    title: &str,
    url: &str,
    ytdlp: &Path,
    equalizer: &EqualizerPayload,
) -> Vec<String> {
    let volume = volume.clamp(0.0, 1.0) * 100.0;
    let title = if title.trim().is_empty() {
        "Yolite".to_string()
    } else {
        title.trim().to_string()
    };
    let mut args = vec![
        "--no-video".to_string(),
        "--no-terminal".to_string(),
        "--really-quiet".to_string(),
        "--force-window=no".to_string(),
        "--idle=no".to_string(),
        format!("--input-ipc-server={}", ipc_path.display()),
        format!("--ytdl-format={YTDLP_AUDIO_FORMAT}"),
        format!("--script-opts=ytdl_hook-ytdl_path={}", ytdlp.display()),
        format!("--volume={volume:.0}"),
        format!("--title=Yolite - {title}"),
        format!("--force-media-title={title}"),
        url.to_string(),
    ];
    if let Some(filter) = equalizer.mpv_filter() {
        args.insert(args.len() - 1, format!("--af={filter}"));
    }
    args
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn cached_stream_url(state: &AppState, video_id: &str) -> Option<String> {
    let now = now_ms();
    if let Ok(cache) = state.stream_cache.lock() {
        if let Some(item) = cache.get(video_id) {
            if item.expires_at > now {
                return Some(item.url.clone());
            }
        }
    }
    None
}

pub(crate) fn remember_stream_url(state: &AppState, video_id: &str, url: &str) {
    if url.is_empty() {
        return;
    }
    if let Ok(mut cache) = state.stream_cache.lock() {
        cache.insert(
            video_id.to_string(),
            CachedStream {
                url: url.to_string(),
                expires_at: now_ms() + STREAM_CACHE_TTL_MS,
            },
        );
    }
}

pub(crate) fn resolve_stream_url_for_cache(
    app: &tauri::AppHandle,
    video_id: &str,
) -> Result<String, String> {
    let config = read_config();
    match ytdlp_resolve_url(app, video_id, None) {
        Ok(stream_url) => Ok(stream_url),
        Err(anonymous_error) if !config.cookies.is_empty() => {
            ytdlp_resolve_url(app, video_id, Some(&config.cookies)).map_err(|cookie_error| {
                format!(
                    "yt-dlp failed without cookies: {anonymous_error}; with cookies: {cookie_error}"
                )
            })
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn native_playback_url<F>(
    state: &AppState,
    video_id: &str,
    resolve: F,
) -> Result<String, String>
where
    F: FnOnce() -> Result<String, String>,
{
    if let Some(stream_url) = cached_stream_url(state, video_id) {
        return Ok(stream_url);
    }

    let stream_url = resolve()?;
    remember_stream_url(state, video_id, &stream_url);
    Ok(stream_url)
}

pub(crate) fn spawn_native_playback(
    app: &tauri::AppHandle,
    _video_id: &str,
    url: &str,
    volume: f64,
    equalizer: &EqualizerPayload,
    title: &str,
    artist: &str,
) -> Result<NativePlayback, String> {
    let player =
        find_mpv_binary().ok_or_else(|| "Could not find mpv for native playback".to_string())?;
    let ytdlp = ytdlp_path(app)?;
    let ipc_path = std::env::temp_dir().join(format!(
        "yolite-mpv-{}-{}.sock",
        std::process::id(),
        COOKIE_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_file(&ipc_path);
    let display_title = if artist.trim().is_empty() {
        title.to_string()
    } else {
        format!("{title} - {artist}")
    };

    let player_child = external_command(&player)
        .args(player_args(
            &ipc_path,
            volume,
            &display_title,
            url,
            &ytdlp,
            equalizer,
        ))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Failed to start {}: {error}", player.display()))?;

    Ok(NativePlayback {
        player: player_child,
        ipc_path,
    })
}

#[tauri::command]
pub(crate) fn play_track_native(
    app: tauri::AppHandle,
    video_id: String,
    volume: f64,
    fade_in: f64,
    equalizer: EqualizerPayload,
    title: String,
    artist: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if !valid_video_id(&video_id) {
        return Err("Invalid video id".to_string());
    }

    let playback_url = native_playback_url(&state, &video_id, || {
        resolve_stream_url_for_cache(&app, &video_id)
    })?;
    let old_playback = if fade_in > 0.0 {
        state
            .playback
            .lock()
            .map_err(|_| "Could not lock playback state".to_string())?
            .take()
    } else {
        stop_playback(&state);
        None
    };
    let playback_volume = if old_playback.is_some() { 0.0 } else { volume };
    let playback = spawn_native_playback(
        &app,
        &video_id,
        &playback_url,
        playback_volume,
        &equalizer,
        &title,
        &artist,
    )?;
    let new_ipc_path = playback.ipc_path.clone();
    let mut target = state
        .playback
        .lock()
        .map_err(|_| "Could not lock playback state".to_string())?;
    *target = Some(playback);
    if let Some(old) = old_playback {
        fade_between_players(old, new_ipc_path, volume, fade_in);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn stop_native_playback(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    stop_playback(&state);
    Ok(())
}

#[cfg(unix)]
pub(crate) fn mpv_ipc_command(ipc_path: &Path, command: Value) -> Result<Value, String> {
    let mut stream = UnixStream::connect(ipc_path).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_millis(650)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_millis(650)))
        .map_err(|error| error.to_string())?;
    let body =
        serde_json::to_string(&json!({ "command": command })).map_err(|error| error.to_string())?;
    stream
        .write_all(format!("{body}\n").as_bytes())
        .map_err(|error| error.to_string())?;
    let mut raw = String::new();
    BufReader::new(stream)
        .read_line(&mut raw)
        .map_err(|error| error.to_string())?;
    serde_json::from_str(raw.trim()).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
pub(crate) fn mpv_ipc_command(_ipc_path: &Path, _command: Value) -> Result<Value, String> {
    Err("Native playback controls require Unix mpv IPC".to_string())
}

pub(crate) fn playback_ipc_path(state: &AppState) -> Result<Option<PathBuf>, String> {
    let mut guard = state
        .playback
        .lock()
        .map_err(|_| "Could not lock playback state".to_string())?;

    let Some(playback) = guard.as_mut() else {
        return Ok(None);
    };

    if playback
        .player
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        let ipc_path = playback.ipc_path.clone();
        let _ = fs::remove_file(&ipc_path);
        *guard = None;
        return Ok(None);
    }

    Ok(Some(playback.ipc_path.clone()))
}

pub(crate) fn mpv_property_number(ipc_path: &Path, property: &str) -> f64 {
    mpv_ipc_command(ipc_path, json!(["get_property", property]))
        .ok()
        .and_then(|payload| {
            payload.get("data").and_then(Value::as_f64).or_else(|| {
                payload
                    .get("data")
                    .and_then(Value::as_u64)
                    .map(|value| value as f64)
            })
        })
        .unwrap_or(0.0)
}

pub(crate) fn mpv_property_bool(ipc_path: &Path, property: &str) -> bool {
    mpv_ipc_command(ipc_path, json!(["get_property", property]))
        .ok()
        .and_then(|payload| payload.get("data").and_then(Value::as_bool))
        .unwrap_or(false)
}

#[tauri::command]
pub(crate) fn get_native_playback(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<PlaybackPayload, String> {
    let Some(ipc_path) = playback_ipc_path(&state)? else {
        return Ok(PlaybackPayload {
            playing: false,
            paused: false,
            position: 0.0,
            duration: 0.0,
        });
    };

    Ok(PlaybackPayload {
        playing: true,
        paused: mpv_property_bool(&ipc_path, "pause"),
        position: mpv_property_number(&ipc_path, "time-pos"),
        duration: mpv_property_number(&ipc_path, "duration"),
    })
}

#[tauri::command]
pub(crate) fn set_native_volume(
    volume: f64,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let Some(ipc_path) = playback_ipc_path(&state)? else {
        return Ok(());
    };
    let volume = volume.clamp(0.0, 1.0) * 100.0;
    mpv_ipc_command(&ipc_path, json!(["set_property", "volume", volume])).map(|_| ())
}

#[tauri::command]
pub(crate) fn set_native_equalizer(
    equalizer: EqualizerPayload,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let Some(ipc_path) = playback_ipc_path(&state)? else {
        return Ok(());
    };
    mpv_ipc_command(
        &ipc_path,
        json!([
            "set_property_string",
            "af",
            equalizer.mpv_filter().unwrap_or_default()
        ]),
    )
    .map(|_| ())
}

#[tauri::command]
pub(crate) fn set_native_pause(
    paused: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let Some(ipc_path) = playback_ipc_path(&state)? else {
        return Ok(());
    };
    mpv_ipc_command(&ipc_path, json!(["set_property", "pause", paused])).map(|_| ())
}

#[tauri::command]
pub(crate) fn seek_native_playback(
    position: f64,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let Some(ipc_path) = playback_ipc_path(&state)? else {
        return Ok(());
    };
    mpv_ipc_command(
        &ipc_path,
        json!(["set_property", "time-pos", position.max(0.0)]),
    )
    .map(|_| ())
}

#[tauri::command]
pub(crate) async fn resolve_track(
    app: tauri::AppHandle,
    video_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<ResolvePayload, String> {
    if !valid_video_id(&video_id) {
        return Err("Invalid video id".to_string());
    }

    let config = read_config();
    let anonymous_result = ytdlp_resolve_url(&app, &video_id, None);
    let (stream_url, uses_cookies) = match anonymous_result {
        Ok(stream_url) => (stream_url, false),
        Err(anonymous_error) if !config.cookies.is_empty() => {
            let stream_url = ytdlp_resolve_url(&app, &video_id, Some(&config.cookies)).map_err(
                |cookie_error| {
                    format!(
                        "yt-dlp failed without cookies: {anonymous_error}; with cookies: {cookie_error}"
                    )
                },
            )?;
            (stream_url, true)
        }
        Err(error) => return Err(error),
    };

    let fallback_url = state
        .stream_base_url
        .lock()
        .ok()
        .and_then(|base| {
            base.as_ref().map(|base| {
                if uses_cookies {
                    format!("{base}/stream/{video_id}?auth=1")
                } else {
                    format!("{base}/stream/{video_id}")
                }
            })
        })
        .unwrap_or_default();

    remember_stream_url(&state, &video_id, &stream_url);

    Ok(ResolvePayload {
        stream_url,
        fallback_url,
    })
}

#[tauri::command]
pub(crate) fn prefetch_track(
    app: tauri::AppHandle,
    video_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if !valid_video_id(&video_id) {
        return Err("Invalid video id".to_string());
    }
    if cached_stream_url(&state, &video_id).is_some() {
        return Ok(());
    }
    let state = state.inner().clone();
    thread::spawn(move || {
        if let Ok(stream_url) = resolve_stream_url_for_cache(&app, &video_id) {
            remember_stream_url(&state, &video_id, &stream_url);
        }
    });
    Ok(())
}

pub(crate) fn ytdlp_resolve_url(
    app: &tauri::AppHandle,
    video_id: &str,
    cookies: Option<&str>,
) -> Result<String, String> {
    let target = format!("https://www.youtube.com/watch?v={video_id}");
    let ytdlp = ytdlp_path(app)?;
    let cookie_file = TempCookieFile::new(cookies.unwrap_or(""))?;
    let mut command = external_command(&ytdlp);
    command.args([
        "--format",
        YTDLP_AUDIO_FORMAT,
        "--no-playlist",
        "--no-warnings",
        "--quiet",
        "--get-url",
        "--add-header",
        "referer:https://music.youtube.com",
        "--add-header",
        "user-agent:Mozilla/5.0",
    ]);

    if let Some(cookie_file) = cookie_file.as_ref() {
        command.args(["--cookies", &cookie_file.path.display().to_string()]);
    }

    let output = command
        .arg(target)
        .output()
        .map_err(|error| format!("Failed to start yt-dlp: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "yt-dlp could not resolve a playable stream".to_string()
        } else {
            stderr
        });
    }

    let stream_url = String::from_utf8_lossy(&output.stdout)
        .lines()
        .last()
        .unwrap_or_default()
        .trim()
        .to_string();

    if stream_url.is_empty() {
        return Err("No playable stream found".to_string());
    }

    Ok(stream_url)
}
