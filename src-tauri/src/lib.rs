use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use qrcode::{QrCode, render::svg};
use reqwest::{Client, header};
use ring::digest::{SHA1_FOR_LEGACY_USE_ONLY, digest};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tokio::sync::Mutex as AsyncMutex;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    cookies: String,
    #[serde(default)]
    auth_user: u8,
    #[serde(default, alias = "userId")]
    user_id: String,
    #[serde(default, alias = "channelId")]
    channel_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionPayload {
    logged_in: bool,
    cookie_bytes: usize,
    config_path: String,
    auth_user: u8,
    user_id: String,
    channel_id: String,
    library_authenticated: Option<bool>,
    profile_name: String,
    profile_picture: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteTrack {
    id: String,
    title: String,
    artist: String,
    thumbnail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemotePlaybackState {
    track: Option<RemoteTrack>,
    playing: bool,
    volume: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteControllerPayload {
    url: String,
    qr_svg: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginPayload {
    id: String,
    name: String,
    version: String,
    description: String,
    css: String,
    discover_categories: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginManifest {
    name: String,
    #[serde(default = "default_plugin_version")]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    discover_categories: Vec<String>,
}

fn default_plugin_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackPayload {
    id: String,
    kind: String,
    title: String,
    artist: String,
    artist_id: Option<String>,
    album: String,
    duration: u64,
    thumbnail: String,
    url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    playlist_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    playlist_params: String,
}

#[derive(Debug, Clone, Serialize)]
struct SearchPayload {
    results: Vec<TrackPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibrarySectionPayload {
    title: String,
    tracks: Vec<TrackPayload>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    layout: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistPayload {
    id: String,
    title: String,
    thumbnail: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryPayload {
    needs_login: bool,
    sections: Vec<LibrarySectionPayload>,
    playlists: Vec<PlaylistPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistDetailPayload {
    tracks: Vec<TrackPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolvePayload {
    stream_url: String,
    fallback_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackPayload {
    playing: bool,
    paused: bool,
    position: f64,
    duration: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EqualizerPayload {
    #[serde(default)]
    preamp: f64,
    #[serde(default)]
    bass: f64,
    #[serde(default)]
    low_mid: f64,
    #[serde(default)]
    mid: f64,
    #[serde(default)]
    high_mid: f64,
    #[serde(default)]
    treble: f64,
    #[serde(default)]
    normalization: bool,
}

impl EqualizerPayload {
    fn db(value: f64) -> f64 {
        value.clamp(-12.0, 12.0)
    }

    fn is_flat(&self) -> bool {
        [
            self.preamp,
            self.bass,
            self.low_mid,
            self.mid,
            self.high_mid,
            self.treble,
        ]
        .into_iter()
        .all(|value| Self::db(value).abs() < f64::EPSILON)
    }

    fn mpv_filter(&self) -> Option<String> {
        if self.is_flat() && !self.normalization {
            return None;
        }
        let preamp = 10_f64.powf(Self::db(self.preamp) / 20.0);
        let normalize = if self.normalization {
            ",dynaudnorm=f=150:g=9:p=0.85:m=8"
        } else {
            ""
        };
        Some(format!(
            "lavfi=[bass=g={:.1}:f=90:w=0.8,equalizer=f=250:t=q:w=1:g={:.1},equalizer=f=1000:t=q:w=1:g={:.1},equalizer=f=4000:t=q:w=1:g={:.1},treble=g={:.1}:f=10000:w=0.8,volume={:.4}{normalize}]",
            Self::db(self.bass),
            Self::db(self.low_mid),
            Self::db(self.mid),
            Self::db(self.high_mid),
            Self::db(self.treble),
            preamp
        ))
    }
}

struct NativePlayback {
    player: std::process::Child,
    ipc_path: PathBuf,
}

struct CachedStream {
    url: String,
    expires_at: u64,
}

#[derive(Default)]
struct AppState {
    client: AsyncMutex<Option<NativeMusicClient>>,
    stream_base_url: std::sync::Mutex<Option<String>>,
    stream_cache: std::sync::Mutex<std::collections::HashMap<String, CachedStream>>,
    playback: std::sync::Mutex<Option<NativePlayback>>,
    discord: std::sync::Mutex<Option<DiscordIpcClient>>,
    remote_state: std::sync::Mutex<RemotePlaybackState>,
    controller_url: std::sync::Mutex<Option<String>>,
    main_window_size: std::sync::Mutex<Option<tauri::PhysicalSize<u32>>>,
}

const DISCORD_APP_ID: &str = "1549900591088541827";

fn config_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir().ok_or_else(|| "Could not find config directory".to_string())?;
    Ok(base.join("yolite").join("config.json"))
}

fn read_config() -> Config {
    let Ok(path) = config_path() else {
        return Config::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Config::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_config(config: &Config) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let body = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(path, body).map_err(|error| error.to_string())
}

const MAX_COOKIE_HEADER_BYTES: usize = 8 * 1024;
const MUSIC_ORIGIN: &str = "https://music.youtube.com";
const MUSIC_ACCOUNT_HINT_PLAYLISTS: &[&str] = &[
    "Liked Music",
    "2025 Recap",
    "Jordy playlist",
    "Dancing",
    "Liked songs (Spotify)",
    "ROCK",
    "SPACE KEES",
];
const YTDLP_AUDIO_FORMAT: &str =
    "bestaudio[ext=m4a]/bestaudio[protocol^=http]/bestaudio/best[protocol^=http]/best";
const STREAM_CACHE_TTL_MS: u64 = 45 * 60 * 1000;
static COOKIE_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn is_youtube_cookie_domain(domain: &str) -> bool {
    let domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    domain == "youtube.com" || domain.ends_with(".youtube.com")
}

fn is_supported_cookie_domain(domain: &str) -> bool {
    let domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    is_youtube_cookie_domain(&domain) || domain == "google.com" || domain.ends_with(".google.com")
}

fn is_likely_youtube_cookie_name(name: &str) -> bool {
    matches!(
        name,
        "APISID"
            | "CONSENT"
            | "DEVICE_INFO"
            | "GPS"
            | "HSID"
            | "LOGIN_INFO"
            | "PREF"
            | "SAPISID"
            | "SID"
            | "SIDCC"
            | "SOCS"
            | "SSID"
            | "VISITOR_INFO1_LIVE"
            | "VISITOR_PRIVACY_METADATA"
            | "YSC"
    ) || name.starts_with("__Secure-1P")
        || name.starts_with("__Secure-3P")
        || name.starts_with("__Secure-Y")
        || name.starts_with("ST-")
}

fn is_music_auth_cookie_name(name: &str) -> bool {
    matches!(
        name,
        "AEC"
            | "APISID"
            | "CONSENT"
            | "DEVICE_INFO"
            | "GPS"
            | "HSID"
            | "LOGIN_INFO"
            | "NID"
            | "PREF"
            | "SAPISID"
            | "SID"
            | "SIDCC"
            | "SOCS"
            | "SSID"
            | "VISITOR_INFO1_LIVE"
            | "VISITOR_PRIVACY_METADATA"
            | "YSC"
    ) || name.starts_with("__Secure-1P")
        || name.starts_with("__Secure-3P")
        || name.starts_with("__Secure-Y")
}

fn cookie_pair(name: &str, value: &str) -> Option<String> {
    let name = name.trim();
    let value = value.trim();
    (!name.is_empty() && !value.is_empty()).then(|| format!("{name}={value}"))
}

fn cookie_name(pair: &str) -> &str {
    pair.split_once('=').map(|(name, _)| name).unwrap_or(pair)
}

fn cookie_value<'a>(cookies: &'a str, name: &str) -> Option<&'a str> {
    cookies
        .split(';')
        .map(str::trim)
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(cookie_name, value)| (cookie_name.trim() == name).then_some(value.trim()))
}

fn youtube_auth_header(cookies: &str) -> Option<String> {
    let sapisid = cookie_value(cookies, "SAPISID")
        .or_else(|| cookie_value(cookies, "__Secure-3PAPISID"))
        .or_else(|| cookie_value(cookies, "__Secure-1PAPISID"))?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let input = format!("{timestamp} {sapisid} {MUSIC_ORIGIN}");
    let hash = digest(&SHA1_FOR_LEGACY_USE_ONLY, input.as_bytes());
    let hash = hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Some(format!("SAPISIDHASH {timestamp}_{hash}"))
}

fn join_cookie_pairs(pairs: Vec<String>) -> String {
    let mut ordered = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for pair in pairs.into_iter().rev() {
        if seen.insert(cookie_name(&pair).to_string()) {
            ordered.push(pair);
        }
    }
    ordered.reverse();
    ordered.join("; ")
}

fn compact_cookie_header(pairs: Vec<String>) -> String {
    let joined = join_cookie_pairs(pairs.clone());
    if joined.len() <= MAX_COOKIE_HEADER_BYTES {
        return joined;
    }

    join_cookie_pairs(
        pairs
            .into_iter()
            .filter(|pair| is_likely_youtube_cookie_name(cookie_name(pair)))
            .collect(),
    )
}

fn firefox_cookie_databases() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let Some(home) = dirs::home_dir() else {
        return paths;
    };
    for base in [
        home.join(".config/mozilla/firefox"),
        home.join(".mozilla/firefox"),
    ] {
        let Ok(entries) = fs::read_dir(base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path().join("cookies.sqlite");
            if path.is_file() {
                paths.push(path);
            }
        }
    }
    paths
}

fn import_firefox_cookies_direct() -> Result<String, String> {
    let databases = firefox_cookie_databases();
    if databases.is_empty() {
        return Err("Could not find Firefox cookies database".to_string());
    }

    let sqlite = Command::new("sqlite3")
        .arg("-version")
        .output()
        .map_err(|_| "sqlite3 is required to read Firefox cookies".to_string())?;
    if !sqlite.status.success() {
        return Err("sqlite3 is required to read Firefox cookies".to_string());
    }

    let mut best = String::new();
    for database in databases {
        let copy_path = std::env::temp_dir().join(format!(
            "yolite-firefox-cookies-{}-{}.sqlite",
            std::process::id(),
            COOKIE_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::copy(&database, &copy_path).map_err(|error| error.to_string())?;
        let output = Command::new("sqlite3")
            .args([
                copy_path.display().to_string(),
                "select host, name, value from moz_cookies where host like '%youtube.com' or host like '%google.com' order by case when host like '%youtube.com' then 1 else 0 end, host, name;".to_string(),
            ])
            .output()
            .map_err(|error| error.to_string())?;
        let _ = fs::remove_file(&copy_path);
        if !output.status.success() {
            continue;
        }

        let pairs = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(3, '|');
                let host = parts.next()?;
                let name = parts.next()?;
                let value = parts.next()?;
                (is_supported_cookie_domain(host) && is_music_auth_cookie_name(name))
                    .then(|| cookie_pair(name, value))
                    .flatten()
            })
            .collect::<Vec<_>>();
        let cookies = join_cookie_pairs(pairs);
        if cookies.len() > best.len() {
            best = cookies;
        }
    }

    if best.is_empty() {
        Err("No Firefox YouTube Music cookies found".to_string())
    } else {
        Ok(best)
    }
}

struct TempCookieFile {
    path: PathBuf,
}

impl TempCookieFile {
    fn new(cookies: &str) -> Result<Option<Self>, String> {
        if cookies.trim().is_empty() {
            return Ok(None);
        }

        let nonce = COOKIE_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "yolite-ytdlp-cookies-{}-{timestamp}-{nonce}.txt",
            std::process::id()
        ));
        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs()
            + 30 * 24 * 60 * 60;

        let mut body = String::from("# Netscape HTTP Cookie File\n");
        for pair in cookies
            .split(';')
            .map(str::trim)
            .filter(|pair| !pair.is_empty())
        {
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            if name.trim().is_empty() || value.contains(['\r', '\n', '\t']) {
                continue;
            }
            body.push_str(&format!(
                ".youtube.com\tTRUE\t/\tTRUE\t{expires}\t{}\t{}\n",
                name.trim(),
                value.trim()
            ));
        }

        fs::write(&path, body).map_err(|error| error.to_string())?;
        Ok(Some(Self { path }))
    }
}

impl Drop for TempCookieFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn normalize_cookies(input: &str) -> String {
    let raw = input.trim();
    if raw.is_empty() {
        return String::new();
    }

    let header_pairs = raw
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            line.strip_prefix("cookie:")
                .or_else(|| line.strip_prefix("Cookie:"))
                .map(str::trim)
        })
        .flat_map(|line| line.split(';'))
        .map(str::trim)
        .filter(|part| !part.is_empty() && part.contains('='))
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if !header_pairs.is_empty() {
        return compact_cookie_header(header_pairs);
    }

    let netscape_pairs = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let parts = line.split('\t').collect::<Vec<_>>();
            (parts.len() >= 7 && is_supported_cookie_domain(parts[0]))
                .then(|| cookie_pair(parts[5], parts[6]))
                .flatten()
        })
        .collect::<Vec<_>>();

    if !netscape_pairs.is_empty() {
        return compact_cookie_header(netscape_pairs);
    }

    let pairs = raw
        .strip_prefix("Cookie:")
        .or_else(|| raw.strip_prefix("cookie:"))
        .unwrap_or(raw)
        .replace(['\r', '\n'], "; ")
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty() && part.contains('=') && !part.contains(':'))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    compact_cookie_header(pairs)
}

fn normalize_webview_cookies(cookies: Vec<tauri::webview::Cookie<'static>>) -> String {
    compact_cookie_header(
        cookies
            .into_iter()
            .filter_map(|cookie| {
                let supported_domain = cookie
                    .domain()
                    .map(is_supported_cookie_domain)
                    .unwrap_or_else(|| is_likely_youtube_cookie_name(cookie.name()));
                supported_domain.then(|| cookie_pair(cookie.name(), cookie.value()))?
            })
            .collect(),
    )
}

#[derive(Debug, Clone)]
struct NativeMusicClient {
    client: Client,
    config: Map<String, Value>,
    cookies: String,
    auth_user: u8,
    user_id: String,
    channel_id: String,
}

impl NativeMusicClient {
    async fn init(
        cookies: String,
        auth_user: u8,
        user_id: String,
        channel_id: String,
    ) -> Result<Self, String> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::USER_AGENT, header::HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_4) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/81.0.4044.129 Safari/537.36"));
        headers.insert(
            header::ACCEPT_LANGUAGE,
            header::HeaderValue::from_static("en-US,en;q=0.5"),
        );
        headers.insert(
            header::ACCEPT_ENCODING,
            header::HeaderValue::from_static("gzip"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|error| error.to_string())?;
        let mut request = client.get("https://music.youtube.com/");
        if !cookies.is_empty() {
            request = request.header(header::COOKIE, &cookies);
        }
        let html = request
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .text()
            .await
            .map_err(|error| error.to_string())?;
        let config = extract_yt_config(&html);

        for key in [
            "INNERTUBE_API_VERSION",
            "INNERTUBE_API_KEY",
            "INNERTUBE_CLIENT_NAME",
            "INNERTUBE_CLIENT_VERSION",
        ] {
            if config.get(key).and_then(Value::as_str).is_none() {
                return Err(format!(
                    "Could not initialize YouTube Music config: missing {key}"
                ));
            }
        }

        Ok(Self {
            client,
            config,
            cookies,
            auth_user,
            user_id,
            channel_id,
        })
    }

    async fn request(&self, endpoint: &str, body: Value) -> Result<Value, String> {
        let api_version = config_str(&self.config, "INNERTUBE_API_VERSION")?;
        let api_key = config_str(&self.config, "INNERTUBE_API_KEY")?;
        let client_name = config_str(&self.config, "INNERTUBE_CLIENT_NAME")?;
        let client_version = config_str(&self.config, "INNERTUBE_CLIENT_VERSION")?;

        let mut url = reqwest::Url::parse(&format!(
            "https://music.youtube.com/youtubei/{api_version}/{endpoint}"
        ))
        .map_err(|error| error.to_string())?;
        url.query_pairs_mut()
            .append_pair("alt", "json")
            .append_pair("key", &api_key)
            .append_pair("prettyPrint", "false");

        let gl = self
            .config
            .get("GL")
            .and_then(Value::as_str)
            .unwrap_or("US");
        let hl = self
            .config
            .get("HL")
            .and_then(Value::as_str)
            .unwrap_or("en");
        let mut request_body = json!({
            "context": {
                "capabilities": {},
                "client": {
                    "clientName": client_name,
                    "clientVersion": client_version,
                    "gl": gl,
                    "hl": hl
                },
                "request": {
                    "internalExperimentFlags": [],
                    "sessionIndex": {}
                },
                "user": { "enableSafetyMode": false }
            }
        });
        let user_id = self.user_id.trim();
        if !user_id.is_empty() {
            request_body["context"]["user"]["onBehalfOfUser"] = Value::String(user_id.to_string());
        }
        merge_json(&mut request_body, body);

        let mut request = self
            .client
            .post(url)
            .header("Origin", MUSIC_ORIGIN)
            .header("Referer", format!("{MUSIC_ORIGIN}/"))
            .header("x-origin", MUSIC_ORIGIN)
            .header(
                "X-Goog-Visitor-Id",
                self.config
                    .get("VISITOR_DATA")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            )
            .header("X-YouTube-Client-Name", client_name)
            .header("X-YouTube-Client-Version", &client_version)
            .header("X-YouTube-Utc-Offset", "0")
            .header("X-YouTube-Time-Zone", "UTC")
            .json(&request_body);

        if !self.cookies.is_empty() {
            request = request.header(header::COOKIE, &self.cookies);
            if let Some(auth) = youtube_auth_header(&self.cookies) {
                request = request
                    .header(header::AUTHORIZATION, auth)
                    .header("X-Goog-AuthUser", self.auth_user.to_string());
            }
        }

        request
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json::<Value>()
            .await
            .map_err(|error| error.to_string())
    }

    async fn account_profile(&self) -> Option<(String, String)> {
        if self.cookies.is_empty() {
            return None;
        }
        let data = self.request("account/account_menu", json!({})).await.ok()?;
        let name = [
            traverse_string(&data, &["accountName", "text"]),
            traverse_string(&data, &["accountName", "runs", "text"]),
            traverse_string(&data, &["channelHandle", "text"]),
        ]
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "YouTube Music account".to_string());
        let picture = traverse_list(&data, &["accountPhoto"])
            .into_iter()
            .map(best_thumbnail)
            .find(|value| !value.is_empty())
            .unwrap_or_else(|| best_thumbnail(&data));
        Some((name, picture))
    }

    async fn search_tracks(&self, query: &str) -> Result<Vec<TrackPayload>, String> {
        let songs = self
            .request(
                "search",
                json!({ "query": query, "params": "Eg-KAQwIARAAGAAgACgAMABqChAEEAMQCRAFEAo%3D" }),
            )
            .await;
        let videos = self
            .request(
                "search",
                json!({ "query": query, "params": "Eg-KAQwIABABGAAgACgAMABqChAEEAMQCRAFEAo%3D" }),
            )
            .await;

        let mut errors = Vec::new();
        let mut tracks = Vec::new();

        match songs {
            Ok(data) => tracks.extend(parse_track_items(&data, "SONG")),
            Err(error) => errors.push(format!("songs: {error}")),
        }
        match videos {
            Ok(data) => tracks.extend(parse_track_items(&data, "VIDEO")),
            Err(error) => errors.push(format!("videos: {error}")),
        }

        if tracks.is_empty() && !errors.is_empty() {
            return Err(format!("Search failed: {}", errors.join("; ")));
        }

        Ok(dedupe_tracks(tracks, 36))
    }

    async fn liked_tracks(&self) -> Result<Vec<TrackPayload>, String> {
        let mut errors = Vec::new();
        for browse_id in ["VLLM", "LM"] {
            match self
                .request("browse", json!({ "browseId": browse_id }))
                .await
                .map(|data| parse_track_items(&data, "VIDEO"))
            {
                Ok(tracks) if !tracks.is_empty() => return Ok(dedupe_tracks(tracks, 50)),
                Ok(_) => errors.push(format!("{browse_id}: no tracks")),
                Err(error) => errors.push(format!("{browse_id}: {error}")),
            }
        }
        Err(errors.join("; "))
    }

    async fn home_sections(&self) -> Result<Vec<LibrarySectionPayload>, String> {
        let data = self
            .request("browse", json!({ "browseId": "FEmusic_home" }))
            .await?;
        let mut sections: Vec<LibrarySectionPayload> =
            traverse_list(&data, &["musicCarouselShelfRenderer"])
                .into_iter()
                .filter_map(|section| {
                    let title = carousel_title(section);
                    let tracks = dedupe_tracks(parse_home_track_items(section), 24);
                    (!title.is_empty() && !tracks.is_empty()).then_some(LibrarySectionPayload {
                        title,
                        tracks,
                        layout: "grid".to_string(),
                    })
                })
                .take(8)
                .collect();
        if sections.is_empty() {
            sections = traverse_list(&data, &["sectionListRenderer", "contents"])
                .into_iter()
                .filter_map(|section| {
                    let title = traverse_string(section, &["header", "title", "text"]);
                    let tracks = dedupe_tracks(parse_home_track_items(section), 24);
                    (!title.is_empty() && !tracks.is_empty()).then_some(LibrarySectionPayload {
                        title,
                        tracks,
                        layout: "grid".to_string(),
                    })
                })
                .take(8)
                .collect();
        }
        Ok(sections)
    }

    async fn library_landing(&self) -> Result<Value, String> {
        self.request("browse", json!({ "browseId": "FEmusic_library_landing" }))
            .await
    }

    async fn library_playlists(&self, landing_data: &Value) -> Vec<PlaylistPayload> {
        let mut playlists = parse_playlist_items(landing_data);

        if let Ok(data) = self
            .request("browse", json!({ "browseId": "FEmusic_liked_playlists" }))
            .await
        {
            playlists.extend(parse_playlist_items(&data));
            let mut token = continuation_token(&data);

            while playlists.len() < 250 {
                let Some(current_token) = token else {
                    break;
                };
                let Ok(data) = self
                    .request("browse", json!({ "continuation": current_token }))
                    .await
                else {
                    break;
                };
                let before = playlists.len();
                playlists.extend(parse_playlist_items(&data));
                token = continuation_token(&data);
                if playlists.len() == before {
                    break;
                }
            }
        }

        if !self.channel_id.is_empty() {
            if let Ok(data) = self
                .request("browse", json!({ "browseId": &self.channel_id }))
                .await
            {
                playlists.extend(parse_public_channel_playlists(&data));
                for params in public_channel_playlist_params(&data).into_iter().take(2) {
                    if let Ok(data) = self
                        .request(
                            "browse",
                            json!({ "browseId": &self.channel_id, "params": params }),
                        )
                        .await
                    {
                        playlists.extend(parse_playlist_items(&data));
                        let mut token = continuation_token(&data);
                        while playlists.len() < 250 {
                            let Some(current_token) = token else {
                                break;
                            };
                            let Ok(data) = self
                                .request("browse", json!({ "continuation": current_token }))
                                .await
                            else {
                                break;
                            };
                            let before = playlists.len();
                            playlists.extend(parse_playlist_items(&data));
                            token = continuation_token(&data);
                            if playlists.len() == before {
                                break;
                            }
                        }
                    }
                }
            }
        }

        dedupe_playlists(playlists, 250)
    }

    async fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<TrackPayload>, String> {
        let playlist_id = normalize_playlist_id(playlist_id)?;
        let data = self
            .request("browse", json!({ "browseId": format!("VL{playlist_id}") }))
            .await?;
        let mut tracks = parse_track_items(&data, "SONG");
        let mut token = continuation_token(&data);

        while tracks.len() < 5000 {
            let Some(current_token) = token else {
                break;
            };
            let data = self
                .request("browse", json!({ "continuation": current_token }))
                .await?;
            let before = tracks.len();
            tracks.extend(parse_track_items(&data, "SONG"));
            token = continuation_token(&data);
            if tracks.len() == before {
                break;
            }
        }

        Ok(dedupe_tracks(tracks, 5000))
    }

    async fn mix_tracks(
        &self,
        video_id: &str,
        playlist_id: &str,
        playlist_params: &str,
    ) -> Result<Vec<TrackPayload>, String> {
        let mut body = json!({ "videoId": video_id });
        if !playlist_id.is_empty() {
            body["playlistId"] = Value::String(playlist_id.to_string());
        }
        if !playlist_params.is_empty() {
            body["params"] = Value::String(playlist_params.to_string());
        }
        let data = self.request("next", body).await?;
        let mut tracks: Vec<TrackPayload> = traverse_list(&data, &["playlistPanelVideoRenderer"])
            .into_iter()
            .filter_map(parse_playlist_panel_track_item)
            .collect();
        tracks.extend(parse_track_items(&data, "SONG"));
        if tracks.is_empty() {
            tracks.extend(parse_track_items(&data, "VIDEO"));
        }
        if let Some(index) = tracks.iter().position(|track| track.id == video_id) {
            tracks.rotate_left(index);
        }
        Ok(dedupe_mix_tracks(tracks, 80))
    }

    async fn set_track_liked(&self, video_id: &str, liked: bool) -> Result<(), String> {
        let endpoint = if liked {
            "like/like"
        } else {
            "like/removelike"
        };
        self.request(endpoint, json!({ "target": { "videoId": video_id } }))
            .await
            .map(|_| ())
    }

    async fn create_playlist(&self, title: &str) -> Result<PlaylistPayload, String> {
        let title = validate_playlist_title(title)?;
        let data = self
            .request(
                "playlist/create",
                json!({ "title": title, "privacyStatus": "PRIVATE" }),
            )
            .await?;
        let id = traverse_string(&data, &["playlistId"]);
        if id.is_empty() {
            return Err("YouTube Music did not return a playlist id".to_string());
        }
        Ok(PlaylistPayload {
            url: format!("https://music.youtube.com/playlist?list={id}"),
            id,
            title: title.to_string(),
            thumbnail: String::new(),
        })
    }

    async fn add_track_to_playlist(&self, playlist_id: &str, video_id: &str) -> Result<(), String> {
        let playlist_id = normalize_playlist_id(playlist_id)?;
        self.request(
            "browse/edit_playlist",
            json!({
                "playlistId": playlist_id,
                "actions": [{
                    "action": "ACTION_ADD_VIDEO",
                    "addedVideoId": video_id
                }]
            }),
        )
        .await
        .map(|_| ())
    }
}

async fn get_client(state: &AppState) -> Result<NativeMusicClient, String> {
    let mut guard = state.client.lock().await;
    if let Some(client) = guard.as_ref() {
        return Ok(client.clone());
    }

    let mut config = read_config();
    if !config.cookies.is_empty() && config.user_id.trim().is_empty() {
        if let Some(user_id) = detect_user_ids_from_account_page(&config.cookies)
            .await
            .into_iter()
            .next()
        {
            if let Some(auth_user) = authenticated_auth_user(&config.cookies, &user_id).await {
                config.user_id = user_id;
                config.auth_user = auth_user;
                let _ = write_config(&config);
            }
        }
    }
    let client = NativeMusicClient::init(
        config.cookies,
        config.auth_user,
        config.user_id,
        config.channel_id,
    )
    .await?;
    *guard = Some(client.clone());
    Ok(client)
}

fn extract_yt_config(html: &str) -> Map<String, Value> {
    let mut config = Map::new();
    let mut offset = 0;
    while let Some(start) = html[offset..].find("ytcfg.set(") {
        let value_start = offset + start + "ytcfg.set(".len();
        if let Some((raw, next_offset)) = extract_json_object(html, value_start) {
            if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&raw) {
                for (key, value) in map {
                    config.insert(key, value);
                }
            }
            offset = next_offset;
        } else {
            break;
        }
    }
    config
}

fn extract_json_object(source: &str, start: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();
    let mut index = start;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if bytes.get(index).copied() != Some(b'{') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (relative, ch) in source[index..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = index + relative + ch.len_utf8();
                    return Some((source[index..end].to_string(), end));
                }
            }
            _ => {}
        }
    }

    None
}

fn config_str(config: &Map<String, Value>, key: &'static str) -> Result<String, String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing config key: {key}"))
}

fn merge_json(target: &mut Value, source: Value) {
    match (target, source) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in b {
                merge_json(a.entry(key).or_insert(Value::Null), value);
            }
        }
        (target, source) => *target = source,
    }
}

fn values_for_key<'a>(data: &'a Value, key: &str, dead_end: bool, out: &mut Vec<&'a Value>) {
    if let Value::Object(map) = data {
        if let Some(value) = map.get(key) {
            out.push(value);
            if dead_end {
                return;
            }
        }
    }

    match data {
        Value::Array(items) => {
            for item in items {
                values_for_key(item, key, false, out);
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                values_for_key(value, key, false, out);
            }
        }
        _ => {}
    }
}

fn traverse_list<'a>(data: &'a Value, keys: &[&str]) -> Vec<&'a Value> {
    let mut current = vec![data];
    for (index, key) in keys.iter().enumerate() {
        let mut next = Vec::new();
        for value in current {
            values_for_key(value, key, index + 1 == keys.len(), &mut next);
        }
        current = next;
    }
    current
}

fn traverse_string(data: &Value, keys: &[&str]) -> String {
    traverse_list(data, keys)
        .into_iter()
        .find_map(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_default()
}

fn is_artist(value: &Value) -> bool {
    matches!(
        traverse_string(value, &["pageType"]).as_str(),
        "MUSIC_PAGE_TYPE_USER_CHANNEL" | "MUSIC_PAGE_TYPE_ARTIST"
    )
}

fn is_album(value: &Value) -> bool {
    traverse_string(value, &["pageType"]) == "MUSIC_PAGE_TYPE_ALBUM"
}

fn is_duration(value: &Value) -> bool {
    let text = value.get("text").and_then(Value::as_str).unwrap_or("");
    !text.is_empty()
        && text.chars().all(|ch| ch.is_ascii_digit() || ch == ':')
        && text.contains(':')
}

fn is_title(value: &Value) -> bool {
    traverse_string(value, &["musicVideoType"]).starts_with("MUSIC_VIDEO_TYPE_")
}

fn parse_duration(value: Option<&str>) -> u64 {
    let Some(value) = value else {
        return 0;
    };
    let mut seconds = 0;
    let mut multiplier = 1;
    for part in value.split(':').rev() {
        let Ok(number) = part.parse::<u64>() else {
            return 0;
        };
        seconds += number * multiplier;
        multiplier *= 60;
    }
    seconds
}

fn collect_thumbnail_items(data: &Value, out: &mut Vec<(u64, String)>) {
    match data {
        Value::Array(items) => {
            for item in items {
                collect_thumbnail_items(item, out);
            }
        }
        Value::Object(map) => {
            if let Some(url) = map.get("url").and_then(Value::as_str) {
                out.push((
                    map.get("width").and_then(Value::as_u64).unwrap_or(0),
                    url.to_string(),
                ));
            }
            for value in map.values() {
                collect_thumbnail_items(value, out);
            }
        }
        _ => {}
    }
}

fn best_thumbnail(data: &Value) -> String {
    let mut items = Vec::new();
    collect_thumbnail_items(data, &mut items);
    items
        .into_iter()
        .max_by_key(|(width, _)| *width)
        .map(|(_, url)| url)
        .unwrap_or_default()
}

fn first_text(data: &Value, keys: &[&str]) -> String {
    traverse_string(data, keys)
}

fn parse_track_item(item: &Value, kind: &str) -> Option<TrackPayload> {
    let mut columns = traverse_list(item, &["flexColumns", "runs"]);
    columns.extend(traverse_list(item, &["fixedColumns", "runs"]));
    let title = columns
        .iter()
        .copied()
        .find(|value| is_title(value))
        .or_else(|| columns.first().copied())
        .unwrap_or(item);
    let artist = columns
        .iter()
        .copied()
        .find(|value| is_artist(value))
        .or_else(|| columns.get(1).copied())
        .unwrap_or(item);
    let album = columns
        .iter()
        .copied()
        .find(|value| is_album(value))
        .map(|value| first_text(value, &["text"]))
        .unwrap_or_default();
    let duration = columns
        .iter()
        .copied()
        .find(|value| is_duration(value))
        .and_then(|value| value.get("text").and_then(Value::as_str));

    let mut id = if kind == "SONG" {
        traverse_string(item, &["playlistItemData", "videoId"])
    } else {
        traverse_string(item, &["playNavigationEndpoint", "videoId"])
    };
    if id.is_empty() {
        id = traverse_string(item, &["videoId"]);
    }
    if id.is_empty() {
        id = best_thumbnail(item)
            .split("/vi/")
            .nth(1)
            .and_then(|tail| tail.split('/').next())
            .unwrap_or("")
            .to_string();
    }
    if id.is_empty() {
        return None;
    }

    Some(TrackPayload {
        url: format!("https://music.youtube.com/watch?v={id}"),
        id,
        kind: kind.to_string(),
        title: first_text(title, &["text"]),
        artist: first_text(artist, &["text"]),
        artist_id: Some(traverse_string(artist, &["browseId"])).filter(|value| !value.is_empty()),
        album,
        duration: parse_duration(duration),
        thumbnail: best_thumbnail(item),
        playlist_id: String::new(),
        playlist_params: String::new(),
    })
}

fn parse_playlist_panel_track_item(item: &Value) -> Option<TrackPayload> {
    let mut id = traverse_string(item, &["watchEndpoint", "videoId"]);
    if id.is_empty() {
        id = traverse_string(item, &["videoId"]);
    }
    if id.is_empty() {
        return None;
    }
    let title = first_text(item, &["title", "runs", "text"]);
    if title.is_empty() {
        return None;
    }
    let artist = first_text(item, &["longBylineText", "runs", "text"]);
    let duration_text = traverse_string(item, &["lengthText", "runs", "text"]);

    Some(TrackPayload {
        url: format!("https://music.youtube.com/watch?v={id}"),
        id,
        kind: "VIDEO".to_string(),
        title,
        artist: if artist.is_empty() {
            "YouTube Music".to_string()
        } else {
            artist
        },
        artist_id: Some(traverse_string(item, &["longBylineText", "browseId"]))
            .filter(|value| !value.is_empty()),
        album: String::new(),
        duration: parse_duration((!duration_text.is_empty()).then_some(duration_text.as_str())),
        thumbnail: best_thumbnail(item),
        playlist_id: traverse_string(item, &["watchEndpoint", "playlistId"]),
        playlist_params: traverse_string(item, &["watchEndpoint", "params"]),
    })
}

fn parse_track_items(data: &Value, kind: &str) -> Vec<TrackPayload> {
    traverse_list(data, &["musicResponsiveListItemRenderer"])
        .into_iter()
        .filter_map(|item| parse_track_item(item, kind))
        .collect()
}

fn subtitle_artist(item: &Value) -> String {
    traverse_list(item, &["subtitle", "runs"])
        .into_iter()
        .find(|value| is_artist(value))
        .or_else(|| {
            traverse_list(item, &["subtitle", "runs"])
                .into_iter()
                .find(|value| {
                    let text = value.get("text").and_then(Value::as_str).unwrap_or("");
                    !text.trim().is_empty()
                        && text.trim() != "•"
                        && !is_duration(value)
                        && !matches!(text, "Song" | "Video" | "Album" | "Playlist")
                })
        })
        .map(|value| first_text(value, &["text"]))
        .unwrap_or_default()
}

fn parse_two_row_track_item(item: &Value) -> Option<TrackPayload> {
    let id = traverse_string(item, &["watchEndpoint", "videoId"]);
    if id.is_empty() {
        return None;
    }
    let title = first_text(item, &["title", "runs", "text"]);
    if title.is_empty() {
        return None;
    }
    let kind = if traverse_string(item, &["musicVideoType"]) == "MUSIC_VIDEO_TYPE_ATV" {
        "SONG"
    } else {
        "VIDEO"
    };

    Some(TrackPayload {
        url: format!("https://music.youtube.com/watch?v={id}"),
        id,
        kind: kind.to_string(),
        title,
        artist: subtitle_artist(item),
        artist_id: Some(traverse_string(item, &["browseId"])).filter(|value| !value.is_empty()),
        album: String::new(),
        duration: 0,
        thumbnail: best_thumbnail(item),
        playlist_id: traverse_string(item, &["watchEndpoint", "playlistId"]),
        playlist_params: traverse_string(item, &["watchEndpoint", "params"]),
    })
}

fn parse_home_track_items(data: &Value) -> Vec<TrackPayload> {
    let mut tracks = parse_track_items(data, "SONG");
    tracks.extend(
        traverse_list(data, &["musicTwoRowItemRenderer"])
            .into_iter()
            .filter_map(parse_two_row_track_item),
    );
    tracks
}

fn parse_playlist_item(item: &Value) -> Option<PlaylistPayload> {
    let title = first_text(item, &["title", "runs", "text"]);
    let mut id = traverse_string(item, &["playlistId"]);
    if id.is_empty() {
        id = traverse_string(item, &["browseId"]);
    }
    if let Some(stripped) = id.strip_prefix("VL") {
        id = stripped.to_string();
    }
    if title.is_empty() || !is_supported_playlist_id(&id) {
        return None;
    }

    Some(PlaylistPayload {
        url: format!("https://music.youtube.com/playlist?list={id}"),
        id,
        title,
        thumbnail: best_thumbnail(item),
    })
}

fn parse_playlist_items(data: &Value) -> Vec<PlaylistPayload> {
    let mut items = traverse_list(data, &["musicTwoRowItemRenderer"])
        .into_iter()
        .filter_map(parse_playlist_item)
        .collect::<Vec<_>>();
    items.extend(
        traverse_list(data, &["musicResponsiveListItemRenderer"])
            .into_iter()
            .filter_map(parse_playlist_item),
    );
    items
}

fn is_supported_playlist_id(id: &str) -> bool {
    matches!(id, "LM" | "SE")
        || id.starts_with("PL")
        || id.starts_with("RD")
        || id.starts_with("OL")
}

fn carousel_title(carousel: &Value) -> String {
    let title = traverse_string(
        carousel,
        &[
            "musicCarouselShelfBasicHeaderRenderer",
            "title",
            "runs",
            "text",
        ],
    );
    if title.is_empty() {
        traverse_string(carousel, &["header", "title", "runs", "text"])
    } else {
        title
    }
}

fn parse_public_channel_playlists(data: &Value) -> Vec<PlaylistPayload> {
    traverse_list(data, &["musicCarouselShelfRenderer"])
        .into_iter()
        .filter(|carousel| carousel_title(carousel).eq_ignore_ascii_case("playlists"))
        .flat_map(parse_playlist_items)
        .collect()
}

fn public_channel_playlist_params(data: &Value) -> Vec<String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Array(items) => {
                for item in items {
                    walk(item, out);
                }
            }
            Value::Object(map) => {
                if let Some(carousel) = map.get("musicCarouselShelfRenderer") {
                    let title = carousel_title(carousel);
                    if title.eq_ignore_ascii_case("playlists") {
                        let params = traverse_string(
                            carousel,
                            &["navigationEndpoint", "browseEndpoint", "params"],
                        );
                        if !params.is_empty() {
                            out.push(params);
                        }
                    }
                }
                for item in map.values() {
                    walk(item, out);
                }
            }
            _ => {}
        }
    }

    let mut params = Vec::new();
    walk(data, &mut params);
    params.sort();
    params.dedup();
    params
}

fn continuation_token(data: &Value) -> Option<String> {
    Some(traverse_string(data, &["continuationCommand", "token"]))
        .filter(|token| !token.is_empty())
        .or_else(|| {
            Some(traverse_string(data, &["continuation"])).filter(|token| !token.is_empty())
        })
}

fn dedupe_tracks(mut tracks: Vec<TrackPayload>, limit: usize) -> Vec<TrackPayload> {
    let mut seen = std::collections::HashSet::new();
    tracks.retain(|track| !track.id.is_empty() && seen.insert(track.id.clone()));
    tracks.truncate(limit);
    tracks
}

fn dedupe_mix_tracks(mut tracks: Vec<TrackPayload>, limit: usize) -> Vec<TrackPayload> {
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_titles = std::collections::HashSet::new();
    tracks.retain(|track| {
        if track.id.is_empty() || !seen_ids.insert(track.id.clone()) {
            return false;
        }
        let identity = format!(
            "{}::{}",
            track.title.trim().to_lowercase(),
            track.artist.trim().to_lowercase()
        );
        identity == "::" || seen_titles.insert(identity)
    });
    tracks.truncate(limit);
    tracks
}

fn dedupe_playlists(mut playlists: Vec<PlaylistPayload>, limit: usize) -> Vec<PlaylistPayload> {
    let mut seen = std::collections::HashSet::new();
    playlists.retain(|playlist| !playlist.id.is_empty() && seen.insert(playlist.id.clone()));
    playlists.truncate(limit);
    playlists
}

fn library_payload_text(data: &Value) -> Result<String, String> {
    serde_json::to_string(data).map_err(|error| error.to_string())
}

fn library_payload_has_content(data: &Value, raw: &str) -> bool {
    !parse_playlist_items(data).is_empty()
        || !parse_track_items(data, "SONG").is_empty()
        || raw.contains("musicTwoRowItemRenderer")
        || raw.contains("musicResponsiveListItemRenderer")
}

fn library_payload_is_authenticated(data: &Value) -> Result<bool, String> {
    let raw = library_payload_text(data)?;
    if library_payload_has_content(data, &raw) {
        return Ok(true);
    }
    Ok(!raw.contains("Sign in to access"))
}

#[tauri::command]
async fn get_session(state: tauri::State<'_, Arc<AppState>>) -> Result<SessionPayload, String> {
    let config = read_config();
    let path = config_path()?;
    let logged_in = !config.cookies.is_empty();
    let (profile_name, profile_picture) = if logged_in {
        match get_client(&state).await {
            Ok(client) => client.account_profile().await.unwrap_or_default(),
            Err(_) => (String::new(), String::new()),
        }
    } else {
        (String::new(), String::new())
    };
    Ok(SessionPayload {
        logged_in,
        cookie_bytes: config.cookies.len(),
        config_path: path.display().to_string(),
        auth_user: config.auth_user,
        user_id: config.user_id,
        channel_id: config.channel_id,
        library_authenticated: None,
        profile_name,
        profile_picture,
    })
}

fn normalize_user_id(user_id: &str) -> String {
    user_id.trim().trim_matches('/').to_string()
}

fn validate_user_id(user_id: &str) -> Result<String, String> {
    let user_id = normalize_user_id(user_id);
    if user_id.is_empty() {
        return Ok(user_id);
    }
    let is_numeric = user_id.bytes().all(|byte| byte.is_ascii_digit());
    if is_numeric && (20..=22).contains(&user_id.len()) {
        return Ok(user_id);
    }
    Err("YouTube Music rejected this User ID format. For multiple YouTube accounts under one Gmail, switch to the music account, open https://www.youtube.com/account_advanced to confirm the account, then use the numeric Brand Account ID from the /b/<id>/ URL at https://myaccount.google.com/brandaccounts.".to_string())
}

fn normalize_channel_id(channel_id: &str) -> String {
    let value = channel_id.trim().trim_matches('/');
    let value = value
        .rsplit_once("/channel/")
        .map(|(_, id)| id)
        .unwrap_or(value);
    value
        .strip_prefix("MPLA")
        .unwrap_or(value)
        .trim_matches('/')
        .to_string()
}

fn validate_channel_id(channel_id: &str) -> Result<String, String> {
    let channel_id = normalize_channel_id(channel_id);
    if channel_id.is_empty() {
        return Ok(channel_id);
    }
    if channel_id.starts_with("UC")
        && channel_id.len() >= 20
        && channel_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Ok(channel_id);
    }
    Err("Use the Channel ID from https://www.youtube.com/account_advanced. It usually starts with UC. The shorter User ID is not accepted for public playlist lookup.".to_string())
}

fn collect_user_ids_after_marker(source: &str, marker: &str, out: &mut Vec<String>) {
    let mut offset = 0;
    while let Some(index) = source[offset..].find(marker) {
        let start = offset + index + marker.len();
        let id = source[start..]
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
            .collect::<String>();
        if validate_user_id(&id).is_ok_and(|value| !value.is_empty()) {
            out.push(id);
        }
        offset = start;
    }
}

fn extract_user_ids_from_account_page(raw: &str) -> Vec<String> {
    let normalized = raw
        .replace("\\/", "/")
        .replace("\\u002F", "/")
        .replace("\\u002f", "/")
        .replace("%2F", "/")
        .replace("%2f", "/");
    let mut ids = Vec::new();
    collect_user_ids_after_marker(&normalized, "/b/", &mut ids);
    collect_user_ids_after_marker(&normalized, "/channel/", &mut ids);
    ids.sort();
    ids.dedup();
    ids
}

async fn detect_user_ids_from_account_page(cookies: &str) -> Vec<String> {
    if cookies.trim().is_empty() {
        return Vec::new();
    }
    let Ok(client) = Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
        .build()
    else {
        return Vec::new();
    };
    let Ok(response) = client
        .get("https://www.youtube.com/account_advanced")
        .header(header::COOKIE, cookies)
        .send()
        .await
    else {
        return Vec::new();
    };
    let Ok(text) = response.text().await else {
        return Vec::new();
    };
    extract_user_ids_from_account_page(&text)
}

async fn resolve_user_id(
    cookies: &str,
    requested_user_id: &str,
    existing_user_id: &str,
) -> Result<String, String> {
    let user_id = if requested_user_id.trim().is_empty() {
        validate_user_id(existing_user_id)?
    } else {
        validate_user_id(requested_user_id)?
    };
    if !user_id.is_empty() {
        return Ok(user_id);
    }
    Ok(detect_user_ids_from_account_page(cookies)
        .await
        .into_iter()
        .next()
        .unwrap_or_default())
}

fn playlist_hint_score(playlists: &[PlaylistPayload]) -> usize {
    let titles = playlists
        .iter()
        .map(|playlist| playlist.title.to_lowercase())
        .collect::<std::collections::HashSet<_>>();
    MUSIC_ACCOUNT_HINT_PLAYLISTS
        .iter()
        .filter(|title| titles.contains(&title.to_lowercase()))
        .count()
}

async fn authenticated_auth_user(cookies: &str, user_id: &str) -> Option<u8> {
    let mut best = None;
    for auth_user in 0..=5 {
        let Ok(client) = NativeMusicClient::init(
            cookies.to_string(),
            auth_user,
            user_id.to_string(),
            String::new(),
        )
        .await
        else {
            continue;
        };
        let Ok(data) = client.library_landing().await else {
            continue;
        };
        if library_payload_is_authenticated(&data).unwrap_or(false) {
            let playlists = client.library_playlists(&data).await;
            let score = 1 + playlists.len() + playlist_hint_score(&playlists) * 100;
            if best
                .map(|(_, best_score)| score > best_score)
                .unwrap_or(true)
            {
                best = Some((auth_user, score));
            }
        }
    }
    best.map(|(auth_user, _)| auth_user)
}

#[tauri::command]
async fn save_session(
    cookies: String,
    user_id: String,
    channel_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SessionPayload, String> {
    let existing = read_config();
    let normalized_cookies = normalize_cookies(&cookies);
    let cookies = if normalized_cookies.is_empty() {
        existing.cookies
    } else {
        normalized_cookies
    };
    if cookies.is_empty() {
        return Err("Paste a Cookie header or cookies.txt export".to_string());
    }
    let user_id = resolve_user_id(&cookies, &user_id, "").await?;
    let channel_id = validate_channel_id(&channel_id)?;
    let detected_auth_user = authenticated_auth_user(&cookies, &user_id).await;
    if !user_id.is_empty() && detected_auth_user.is_none() {
        return Err("YouTube Music did not accept that User ID for these cookies. Check that you are signed into the same Gmail account and use the numeric Brand Account ID from the /b/<id>/ URL.".to_string());
    }
    let auth_user = detected_auth_user.unwrap_or(0);
    let authenticated = detected_auth_user.is_some();
    write_config(&Config {
        cookies,
        auth_user,
        user_id,
        channel_id,
    })?;
    *state.client.lock().await = None;
    let mut payload = get_session(state).await?;
    payload.library_authenticated = Some(authenticated);
    Ok(payload)
}

#[tauri::command]
async fn clear_session(state: tauri::State<'_, Arc<AppState>>) -> Result<SessionPayload, String> {
    write_config(&Config::default())?;
    *state.client.lock().await = None;
    get_session(state).await
}

#[tauri::command]
async fn search_tracks(
    query: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SearchPayload, String> {
    let query = query.trim();
    if query.len() < 2 {
        return Ok(SearchPayload { results: vec![] });
    }

    let client = get_client(&state).await?;
    Ok(SearchPayload {
        results: client.search_tracks(query).await?,
    })
}

#[tauri::command]
async fn get_library(state: tauri::State<'_, Arc<AppState>>) -> Result<LibraryPayload, String> {
    if read_config().cookies.is_empty() {
        return Ok(LibraryPayload {
            needs_login: true,
            sections: vec![],
            playlists: vec![],
        });
    }

    let client = get_client(&state).await?;
    let library_landing = client.library_landing().await?;
    let landing_raw = library_payload_text(&library_landing)?;
    let playlists = client.library_playlists(&library_landing).await;
    let landing_tracks = dedupe_tracks(parse_track_items(&library_landing, "SONG"), 50);
    if !library_payload_has_content(&library_landing, &landing_raw)
        && !library_payload_is_authenticated(&library_landing)?
    {
        return Ok(LibraryPayload {
            needs_login: true,
            sections: vec![],
            playlists: vec![],
        });
    }

    let mut sections = Vec::new();
    if !landing_tracks.is_empty() {
        sections.push(LibrarySectionPayload {
            title: "Library".to_string(),
            tracks: landing_tracks,
            layout: String::new(),
        });
    }

    if let Ok(tracks) = client.liked_tracks().await {
        if !tracks.is_empty() {
            sections.push(LibrarySectionPayload {
                title: "Liked Music".to_string(),
                tracks,
                layout: String::new(),
            });
        }
    }

    Ok(LibraryPayload {
        needs_login: false,
        sections,
        playlists,
    })
}

#[tauri::command]
async fn get_playlist(
    playlist_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<PlaylistDetailPayload, String> {
    let client = get_client(&state).await?;
    Ok(PlaylistDetailPayload {
        tracks: client.playlist_tracks(&playlist_id).await?,
    })
}

#[tauri::command]
async fn get_mix(
    video_id: String,
    playlist_id: String,
    playlist_params: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<PlaylistDetailPayload, String> {
    if !valid_video_id(&video_id) {
        return Err("Invalid video id".to_string());
    }
    let playlist_id = if playlist_id.trim().is_empty() {
        String::new()
    } else {
        normalize_playlist_id(&playlist_id)?
    };
    let playlist_params = playlist_params.trim();
    if !valid_playlist_params(playlist_params) {
        return Err("Invalid playlist params".to_string());
    }
    let client = get_client(&state).await?;
    Ok(PlaylistDetailPayload {
        tracks: client
            .mix_tracks(&video_id, &playlist_id, playlist_params)
            .await?,
    })
}

#[tauri::command]
async fn get_home(state: tauri::State<'_, Arc<AppState>>) -> Result<LibraryPayload, String> {
    let client = get_client(&state).await?;
    Ok(LibraryPayload {
        needs_login: false,
        sections: client.home_sections().await.unwrap_or_default(),
        playlists: vec![],
    })
}

#[tauri::command]
async fn set_track_liked(
    video_id: String,
    liked: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if !valid_video_id(&video_id) {
        return Err("Invalid video id".to_string());
    }
    let client = get_client(&state).await?;
    client.set_track_liked(&video_id, liked).await
}

#[tauri::command]
async fn create_playlist(
    title: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<PlaylistPayload, String> {
    let client = get_client(&state).await?;
    client.create_playlist(&title).await
}

#[tauri::command]
async fn add_track_to_playlist(
    playlist_id: String,
    video_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if !valid_video_id(&video_id) {
        return Err("Invalid video id".to_string());
    }
    let client = get_client(&state).await?;
    client.add_track_to_playlist(&playlist_id, &video_id).await
}

fn browser_cookie_sources() -> [&'static str; 7] {
    [
        "firefox", "chrome", "chromium", "brave", "edge", "opera", "vivaldi",
    ]
}

const MUSIC_COOKIE_IMPORT_PROBE_URL: &str = "https://music.youtube.com/watch?v=jNQXAC9IVRw";

fn validate_browser_source(browser: &str) -> Result<(), String> {
    if browser_cookie_sources().contains(&browser) {
        Ok(())
    } else {
        Err("Choose a supported browser cookie source".to_string())
    }
}

fn import_cookies_from_browser(app: &tauri::AppHandle, browser: &str) -> Result<String, String> {
    validate_browser_source(browser)?;
    if browser == "firefox" {
        if let Ok(cookies) = import_firefox_cookies_direct() {
            return Ok(cookies);
        }
    }

    let cookie_path = std::env::temp_dir().join(format!(
        "yolite-cookies-{}-{}.txt",
        std::process::id(),
        browser
    ));
    let cookie_path_string = cookie_path.display().to_string();
    let ytdlp = ytdlp_path(app)?;
    let output = external_command(&ytdlp)
        .args([
            "--cookies-from-browser",
            browser,
            "--cookies",
            &cookie_path_string,
            "--skip-download",
            "--simulate",
            "--no-warnings",
            "--quiet",
            "--no-playlist",
            MUSIC_COOKIE_IMPORT_PROBE_URL,
        ])
        .output()
        .map_err(|error| format!("Failed to start yt-dlp: {error}"))?;

    let raw = fs::read_to_string(&cookie_path).unwrap_or_default();
    let _ = fs::remove_file(&cookie_path);
    let cookies = normalize_cookies(&raw);

    if !cookies.is_empty() {
        return Ok(cookies);
    }

    if output.status.success() {
        Err(format!("No YouTube cookies found in {browser}"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("Could not import cookies from {browser}")
        } else {
            stderr
        })
    }
}

#[tauri::command]
fn open_youtube_music() -> Result<(), String> {
    let url = "https://music.youtube.com/";

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open browser: {error}"))
}

#[tauri::command]
async fn open_app_login(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("youtube-login") {
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }

    let url = reqwest::Url::parse("https://music.youtube.com/")
        .map_err(|error| format!("Invalid login URL: {error}"))?;
    tauri::WebviewWindowBuilder::new(&app, "youtube-login", tauri::WebviewUrl::External(url))
        .title("Yolite YouTube Music Login")
        .inner_size(1100.0, 760.0)
        .resizable(true)
        .build()
        .map_err(|error| format!("Could not open in-app login: {error}"))?;
    Ok(())
}

#[tauri::command]
async fn import_app_login_session(
    app: tauri::AppHandle,
    user_id: String,
    channel_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SessionPayload, String> {
    let Some(window) = app.get_webview_window("youtube-login") else {
        return Err(
            "Open the in-app login window first, then sign in or switch YouTube accounts."
                .to_string(),
        );
    };

    let cookies = normalize_webview_cookies(
        window
            .cookies()
            .map_err(|error| format!("Could not read in-app login cookies: {error}"))?,
    );
    if cookies.is_empty() {
        return Err("No YouTube cookies found in the in-app login window yet.".to_string());
    }

    let existing = read_config();
    let user_id = resolve_user_id(&cookies, &user_id, &existing.user_id).await?;
    let Some(auth_user) = authenticated_auth_user(&cookies, &user_id).await else {
        return Err("The in-app login cookies were found, but YouTube Music did not accept them yet. Finish sign-in, switch to the music account in that window, then try again.".to_string());
    };

    let channel_id = if channel_id.trim().is_empty() {
        existing.channel_id
    } else {
        validate_channel_id(&channel_id)?
    };
    write_config(&Config {
        cookies,
        auth_user,
        user_id,
        channel_id,
    })?;
    *state.client.lock().await = None;
    let _ = window.close();
    let mut payload = get_session(state).await?;
    payload.library_authenticated = Some(true);
    Ok(payload)
}

#[tauri::command]
async fn import_browser_session(
    app: tauri::AppHandle,
    browser: String,
    user_id: String,
    channel_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SessionPayload, String> {
    let requested = browser.trim();
    let sources = if requested.eq_ignore_ascii_case("auto") || requested.is_empty() {
        browser_cookie_sources().to_vec()
    } else {
        vec![requested]
    };

    let mut first_cookie_set = None;
    let mut first_user_id = String::new();
    let mut errors = Vec::new();
    let existing = read_config();
    let user_id = if user_id.trim().is_empty() {
        validate_user_id(&existing.user_id)?
    } else {
        validate_user_id(&user_id)?
    };
    let channel_id = validate_channel_id(&channel_id)?;
    for source in sources {
        match import_cookies_from_browser(&app, source) {
            Ok(cookies) => {
                let candidate_user_id = if user_id.is_empty() {
                    resolve_user_id(&cookies, "", "").await?
                } else {
                    user_id.clone()
                };
                if first_cookie_set.is_none() {
                    first_cookie_set = Some(cookies.clone());
                    first_user_id = candidate_user_id.clone();
                }
                if let Some(auth_user) = authenticated_auth_user(&cookies, &candidate_user_id).await
                {
                    write_config(&Config {
                        cookies,
                        auth_user,
                        user_id: candidate_user_id,
                        channel_id: channel_id.clone(),
                    })?;
                    *state.client.lock().await = None;
                    let mut payload = get_session(state).await?;
                    payload.library_authenticated = Some(true);
                    return Ok(payload);
                }
                errors.push(format!("{source}: cookies rejected by YouTube Music"));
            }
            Err(error) => errors.push(format!("{source}: {error}")),
        }
    }

    let Some(cookies) = first_cookie_set else {
        return Err(errors.join("; "));
    };
    if !user_id.is_empty() {
        return Err("Imported cookies were found, but YouTube Music did not accept that User ID. Check that you are signed into the same Gmail account and use the numeric Brand Account ID from the /b/<id>/ URL.".to_string());
    }
    write_config(&Config {
        cookies,
        auth_user: 0,
        user_id: first_user_id,
        channel_id,
    })?;
    *state.client.lock().await = None;
    let mut payload = get_session(state).await?;
    payload.library_authenticated = Some(false);
    Ok(payload)
}

fn is_executable(path: &Path) -> bool {
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

fn find_candidate_ytdlp(app: Option<&tauri::AppHandle>) -> Option<PathBuf> {
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

fn find_ytdlp_binary(app: &tauri::AppHandle) -> Option<PathBuf> {
    find_candidate_ytdlp(Some(app))
}

fn ytdlp_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    find_ytdlp_binary(app).ok_or_else(|| {
        "yt-dlp executable could not be found. Please ensure yt-dlp is installed or run `npm install` to download the bundled binary.".to_string()
    })
}

fn find_mpv_binary() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path_var| {
        std::env::split_paths(&path_var)
            .map(|dir| dir.join("mpv"))
            .find(|path| is_executable(path))
    })
}

fn external_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.env_remove("PYTHONHOME").env_remove("PYTHONPATH");
    command
}

fn stop_playback(state: &AppState) {
    if let Ok(mut playback) = state.playback.lock() {
        if let Some(mut playback) = playback.take() {
            let _ = playback.player.kill();
            let _ = playback.player.wait();
            let _ = fs::remove_file(&playback.ipc_path);
        }
    }
}

fn fade_between_players(
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

fn ytdlp_stream_args(video_id: &str, cookie_file: Option<&Path>) -> Vec<String> {
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

fn player_args(
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn cached_stream_url(state: &AppState, video_id: &str) -> Option<String> {
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

fn remember_stream_url(state: &AppState, video_id: &str, url: &str) {
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

fn resolve_stream_url_for_cache(app: &tauri::AppHandle, video_id: &str) -> Result<String, String> {
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

fn native_playback_url<F>(state: &AppState, video_id: &str, resolve: F) -> Result<String, String>
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

fn spawn_native_playback(
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
fn play_track_native(
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
fn stop_native_playback(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    stop_playback(&state);
    Ok(())
}

#[cfg(unix)]
fn mpv_ipc_command(ipc_path: &Path, command: Value) -> Result<Value, String> {
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
fn mpv_ipc_command(_ipc_path: &Path, _command: Value) -> Result<Value, String> {
    Err("Native playback controls require Unix mpv IPC".to_string())
}

fn playback_ipc_path(state: &AppState) -> Result<Option<PathBuf>, String> {
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

fn mpv_property_number(ipc_path: &Path, property: &str) -> f64 {
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

fn mpv_property_bool(ipc_path: &Path, property: &str) -> bool {
    mpv_ipc_command(ipc_path, json!(["get_property", property]))
        .ok()
        .and_then(|payload| payload.get("data").and_then(Value::as_bool))
        .unwrap_or(false)
}

#[tauri::command]
fn get_native_playback(state: tauri::State<'_, Arc<AppState>>) -> Result<PlaybackPayload, String> {
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
fn set_native_volume(volume: f64, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let Some(ipc_path) = playback_ipc_path(&state)? else {
        return Ok(());
    };
    let volume = volume.clamp(0.0, 1.0) * 100.0;
    mpv_ipc_command(&ipc_path, json!(["set_property", "volume", volume])).map(|_| ())
}

#[tauri::command]
fn set_native_equalizer(
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
fn set_native_pause(paused: bool, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let Some(ipc_path) = playback_ipc_path(&state)? else {
        return Ok(());
    };
    mpv_ipc_command(&ipc_path, json!(["set_property", "pause", paused])).map(|_| ())
}

#[tauri::command]
fn seek_native_playback(
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

fn read_http_request(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = [0; 2048];
    let mut request = Vec::new();

    loop {
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") || request.len() > 8192 {
            break;
        }
    }

    String::from_utf8(request).ok()
}

fn http_text(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\ncache-control: no-store\r\naccess-control-allow-origin: *\r\n\r\n{body}",
        body.len()
    );
}

fn handle_stream_request(mut stream: TcpStream, app: tauri::AppHandle) {
    let Some(request) = read_http_request(&mut stream) else {
        return;
    };
    let Some(first_line) = request.lines().next() else {
        return;
    };
    let parts = first_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 || parts[0] != "GET" {
        return http_text(&mut stream, "405 Method Not Allowed", "Method not allowed");
    }

    let raw_path = parts[1];
    let path = raw_path.split('?').next().unwrap_or("");
    let use_cookies = raw_path
        .split_once('?')
        .map(|(_, query)| query.split('&').any(|part| part == "auth=1"))
        .unwrap_or(false);
    let Some(video_id) = path.strip_prefix("/stream/") else {
        return http_text(&mut stream, "404 Not Found", "Not found");
    };
    if !valid_video_id(video_id) {
        return http_text(&mut stream, "400 Bad Request", "Invalid video id");
    }

    let ytdlp = match ytdlp_path(&app) {
        Ok(path) => path,
        Err(error) => {
            return http_text(
                &mut stream,
                "502 Bad Gateway",
                &format!("Failed to start yt-dlp: {error}"),
            );
        }
    };

    let first_cookie_file = if use_cookies {
        let config = read_config();
        match TempCookieFile::new(&config.cookies) {
            Ok(cookie_file) => cookie_file,
            Err(error) => {
                return http_text(
                    &mut stream,
                    "502 Bad Gateway",
                    &format!("Could not prepare cookies: {error}"),
                );
            }
        }
    } else {
        None
    };
    let first_cookie_path = first_cookie_file
        .as_ref()
        .map(|cookie_file| cookie_file.path.as_path());
    let mut child = match external_command(&ytdlp)
        .args(ytdlp_stream_args(video_id, first_cookie_path))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return http_text(
                &mut stream,
                "502 Bad Gateway",
                &format!("Failed to start yt-dlp: {error}"),
            );
        }
    };

    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        return http_text(&mut stream, "502 Bad Gateway", "No yt-dlp output");
    };

    let _ = stream.write_all(
        b"HTTP/1.1 200 OK\r\ncontent-type: audio/mp4\r\ncache-control: no-store\r\naccess-control-allow-origin: *\r\nconnection: close\r\n\r\n",
    );
    let copied = std::io::copy(&mut stdout, &mut stream).unwrap_or(0);
    let _ = child.wait();

    if copied > 0 {
        return;
    }
    if use_cookies {
        return;
    }

    let config = read_config();
    let Some(cookie_file) = TempCookieFile::new(&config.cookies).ok().flatten() else {
        return;
    };
    let Ok(mut child) = external_command(&ytdlp)
        .args(ytdlp_stream_args(video_id, Some(&cookie_file.path)))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    if let Some(mut stdout) = child.stdout.take() {
        let _ = std::io::copy(&mut stdout, &mut stream);
    }
    let _ = child.wait();
}

fn start_stream_server(app: tauri::AppHandle) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let addr = listener.local_addr().map_err(|error| error.to_string())?;
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            thread::spawn(move || handle_stream_request(stream, app));
        }
    });
    Ok(format!("http://{addr}"))
}

fn secure_token() -> Result<String, String> {
    let mut bytes = [0_u8; 18];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "Could not create controller token".to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn local_network_ip() -> Result<String, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| error.to_string())?;
    socket
        .connect("8.8.8.8:80")
        .map_err(|error| error.to_string())?;
    socket
        .local_addr()
        .map(|address| address.ip().to_string())
        .map_err(|error| error.to_string())
}

fn http_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
}

fn controller_html(token: &str) -> String {
    r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>YoLite remote</title>
<style>
:root{color-scheme:dark;font-family:system-ui,sans-serif}*{box-sizing:border-box}body{margin:0;min-height:100dvh;display:grid;place-items:center;background:#0b0c0d;color:#f4f1eb;padding:22px}.card{width:min(430px,100%);padding:22px;border:1px solid #ffffff18;border-radius:22px;background:#15171ae8;box-shadow:0 26px 80px #0008}.now{display:grid;grid-template-columns:82px 1fr;gap:16px;align-items:center}.now img{width:82px;height:82px;border-radius:14px;object-fit:cover;background:#24282c}.muted{color:#aaa7a1}.controls{display:grid;grid-template-columns:repeat(3,1fr);gap:10px;margin:22px 0 12px}button,a{min-height:52px;border:0;border-radius:13px;background:#24282c;color:#f4f1eb;font:700 16px inherit;text-decoration:none;display:grid;place-items:center}button.primary{background:#f4f1eb;color:#111315}.secondary{display:grid;grid-template-columns:1fr 1fr;gap:10px}.download{margin-top:10px;background:#d38a64;color:#111315}h1{font-size:17px;margin:0 0 18px}strong{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:18px}.muted{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;margin-top:4px}
</style></head>
<body><main class="card"><h1>YoLite remote</h1><section class="now"><img id="art" alt=""><div><strong id="title">Nothing playing</strong><div id="artist" class="muted">Pick a song in YoLite</div></div></section><div class="controls"><button data-action="previous">Previous</button><button class="primary" data-action="playPause">Play / pause</button><button data-action="next">Next</button></div><div class="secondary"><button data-action="volumeDown">Volume down</button><button data-action="volumeUp">Volume up</button></div><a id="download" class="download" hidden>Download song</a></main><script>
const token="__TOKEN__";const title=document.querySelector('#title');const artist=document.querySelector('#artist');const art=document.querySelector('#art');const download=document.querySelector('#download');document.querySelectorAll('[data-action]').forEach(button=>button.onclick=()=>fetch(`/api/control/${token}/${button.dataset.action}`));async function update(){try{const state=await fetch(`/api/state/${token}`).then(response=>response.json());title.textContent=state.track?.title||'Nothing playing';artist.textContent=state.track?.artist||'Pick a song in YoLite';if(state.track?.thumbnail)art.src=state.track.thumbnail;else art.removeAttribute('src');if(state.track?.id){download.hidden=false;download.href=`/download/${token}/${state.track.id}`}else download.hidden=true}catch{}}update();setInterval(update,1500);
</script></body></html>"#
        .replace("__TOKEN__", token)
}

fn safe_download_name(value: &str) -> String {
    let name = value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
        .take(100)
        .collect::<String>()
        .trim()
        .to_string();
    if name.is_empty() {
        "yolite-song".to_string()
    } else {
        name
    }
}

fn handle_controller_download(
    stream: &mut TcpStream,
    app: &tauri::AppHandle,
    state: &AppState,
    video_id: &str,
) {
    if !valid_video_id(video_id) {
        return http_response(stream, "400 Bad Request", "text/plain", b"Invalid video id");
    }
    let config = read_config();
    let cookie_file = TempCookieFile::new(&config.cookies).ok().flatten();
    let ytdlp = match ytdlp_path(app) {
        Ok(path) => path,
        Err(error) => {
            return http_response(stream, "502 Bad Gateway", "text/plain", error.as_bytes());
        }
    };
    let mut child = match external_command(&ytdlp)
        .args(ytdlp_stream_args(
            video_id,
            cookie_file.as_ref().map(|file| file.path.as_path()),
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return http_response(
                stream,
                "502 Bad Gateway",
                "text/plain",
                error.to_string().as_bytes(),
            );
        }
    };
    let title = state
        .remote_state
        .lock()
        .ok()
        .and_then(|remote| remote.track.clone())
        .filter(|track| track.id == video_id)
        .map(|track| safe_download_name(&track.title))
        .unwrap_or_else(|| "yolite-song".to_string());
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\ncontent-type: audio/mp4\r\ncontent-disposition: attachment; filename=\"{title}.m4a\"\r\ncache-control: no-store\r\nconnection: close\r\n\r\n"
    );
    if let Some(mut output) = child.stdout.take() {
        let _ = std::io::copy(&mut output, stream);
    }
    let _ = child.wait();
}

fn handle_controller_request(
    mut stream: TcpStream,
    app: tauri::AppHandle,
    state: Arc<AppState>,
    token: String,
) {
    let Some(request) = read_http_request(&mut stream) else {
        return;
    };
    let Some(path) = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .map(|path| path.split('?').next().unwrap_or(path))
    else {
        return;
    };
    if path == format!("/controller/{token}") {
        return http_response(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            controller_html(&token).as_bytes(),
        );
    }
    if path == format!("/api/state/{token}") {
        let remote = state
            .remote_state
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default();
        let body = serde_json::to_vec(&remote).unwrap_or_else(|_| b"{}".to_vec());
        return http_response(&mut stream, "200 OK", "application/json", &body);
    }
    if let Some(action) = path.strip_prefix(&format!("/api/control/{token}/")) {
        if matches!(
            action,
            "playPause" | "next" | "previous" | "volumeUp" | "volumeDown"
        ) {
            let _ = app.emit("remote-control", action.to_string());
            return http_response(&mut stream, "204 No Content", "text/plain", b"");
        }
    }
    if let Some(video_id) = path.strip_prefix(&format!("/download/{token}/")) {
        return handle_controller_download(&mut stream, &app, &state, video_id);
    }
    http_response(&mut stream, "404 Not Found", "text/plain", b"Not found");
}

fn start_controller_server(app: tauri::AppHandle, state: Arc<AppState>) -> Result<String, String> {
    let listener = TcpListener::bind("0.0.0.0:0").map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let token = secure_token()?;
    let url = format!("http://{}:{port}/controller/{token}", local_network_ip()?);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            let state = state.clone();
            let token = token.clone();
            thread::spawn(move || handle_controller_request(stream, app, state, token));
        }
    });
    Ok(url)
}

fn valid_video_id(video_id: &str) -> bool {
    video_id.len() >= 6
        && video_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn normalize_playlist_id(playlist_id: &str) -> Result<String, String> {
    let id = playlist_id
        .trim()
        .strip_prefix("VL")
        .unwrap_or(playlist_id.trim());
    if id.len() >= 2
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        Ok(id.to_string())
    } else {
        Err("Invalid playlist id".to_string())
    }
}

fn valid_playlist_params(params: &str) -> bool {
    params
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '%')
}

fn validate_playlist_title(title: &str) -> Result<&str, String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 80 {
        return Err("Playlist name must be 1-80 characters".to_string());
    }
    Ok(title)
}

#[tauri::command]
async fn resolve_track(
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
fn prefetch_track(
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

fn ytdlp_resolve_url(
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

#[tauri::command]
fn set_global_shortcuts(
    app: tauri::AppHandle,
    shortcuts: HashMap<String, String>,
) -> Result<(), String> {
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|error| format!("Could not clear global hotkeys: {error}"))?;
    for (action, shortcut) in shortcuts {
        if !matches!(
            action.as_str(),
            "playPause" | "next" | "previous" | "volumeUp" | "volumeDown"
        ) || shortcut.trim().is_empty()
        {
            continue;
        }
        let action_name = action.clone();
        manager
            .on_shortcut(shortcut.trim(), move |app, _, event| {
                if event.state() == ShortcutState::Pressed {
                    let _ = app.emit("global-hotkey", action_name.clone());
                }
            })
            .map_err(|error| format!("Could not register {action}: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
fn set_mini_player(
    app: tauri::AppHandle,
    enabled: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window is unavailable".to_string())?;
    if enabled {
        if let Ok(mut size) = state.main_window_size.lock() {
            *size = window.inner_size().ok();
        }
        window
            .set_min_size(None::<tauri::Size>)
            .map_err(|error| error.to_string())?;
        window
            .set_size(tauri::PhysicalSize::new(480, 150))
            .map_err(|error| error.to_string())?;
        window
            .set_always_on_top(true)
            .map_err(|error| error.to_string())?;
        window
            .set_decorations(false)
            .map_err(|error| error.to_string())?;
    } else {
        window
            .set_decorations(true)
            .map_err(|error| error.to_string())?;
        window
            .set_always_on_top(false)
            .map_err(|error| error.to_string())?;
        window
            .set_min_size(Some(tauri::PhysicalSize::new(860, 560)))
            .map_err(|error| error.to_string())?;
        let size = state
            .main_window_size
            .lock()
            .ok()
            .and_then(|size| *size)
            .unwrap_or_else(|| tauri::PhysicalSize::new(1180, 760));
        window.set_size(size).map_err(|error| error.to_string())?;
    }
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

fn plugin_root() -> Result<PathBuf, String> {
    let config = config_path()?;
    let root = config
        .parent()
        .ok_or_else(|| "Could not find plugin directory".to_string())?
        .join("plugins");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let guide = root.join("README.md");
    if !guide.exists() {
        fs::write(
            guide,
            "# YoLite plugins\n\nCreate one directory per plugin. Add `plugin.json` and optional `theme.css`.\n\n```json\n{\n  \"name\": \"My theme\",\n  \"version\": \"1.0.0\",\n  \"description\": \"Custom YoLite colors\",\n  \"discoverCategories\": [\"Deep focus\"]\n}\n```\n\nPlugins are declarative. Theme CSS runs in YoLite's interface; only install plugins you trust.\n",
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(root)
}

#[tauri::command]
fn load_plugins() -> Result<Vec<PluginPayload>, String> {
    let root = plugin_root()?;
    let mut plugins = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("plugin.json");
        let Ok(raw) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<PluginManifest>(&raw) else {
            continue;
        };
        let id = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("plugin")
            .to_string();
        let css = fs::read_to_string(path.join("theme.css"))
            .unwrap_or_default()
            .chars()
            .take(256 * 1024)
            .collect();
        let discover_categories = manifest
            .discover_categories
            .into_iter()
            .map(|value| value.trim().chars().take(60).collect::<String>())
            .filter(|value| !value.is_empty())
            .take(32)
            .collect();
        plugins.push(PluginPayload {
            id,
            name: if manifest.name.trim().is_empty() {
                "Unnamed plugin".to_string()
            } else {
                manifest.name
            },
            version: manifest.version,
            description: manifest.description,
            css,
            discover_categories,
        });
    }
    plugins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(plugins)
}

#[tauri::command]
fn update_remote_state(
    track: Option<RemoteTrack>,
    playing: bool,
    volume: f64,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut remote = state
        .remote_state
        .lock()
        .map_err(|_| "Remote controller state is unavailable".to_string())?;
    remote.track = track;
    remote.playing = playing;
    remote.volume = volume.clamp(0.0, 1.0);
    Ok(())
}

#[tauri::command]
fn get_remote_controller(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<RemoteControllerPayload, String> {
    let url = state
        .controller_url
        .lock()
        .map_err(|_| "Phone controller state is unavailable".to_string())?
        .clone()
        .ok_or_else(|| "Phone controller is unavailable".to_string())?;
    let code = QrCode::new(url.as_bytes()).map_err(|error| error.to_string())?;
    let qr_svg = code
        .render::<svg::Color>()
        .min_dimensions(240, 240)
        .dark_color(svg::Color("#111315"))
        .light_color(svg::Color("#f7f4ee"))
        .build();
    Ok(RemoteControllerPayload { url, qr_svg })
}

fn discord_text(value: &str, fallback: &str) -> String {
    let text = value.trim();
    let text = if text.is_empty() { fallback } else { text };
    text.chars().take(128).collect()
}

fn connect_discord() -> Result<DiscordIpcClient, String> {
    let mut client = DiscordIpcClient::new(DISCORD_APP_ID);
    client
        .connect()
        .map_err(|error| format!("Could not connect to Discord: {error}"))?;
    Ok(client)
}

#[tauri::command]
fn update_discord_presence(
    state: tauri::State<'_, Arc<AppState>>,
    title: String,
    artist: String,
    video_id: String,
    thumbnail: String,
    paused: bool,
) -> Result<(), String> {
    let details = discord_text(&title, "Unknown song");
    let artist = discord_text(&artist, "Unknown artist");
    let status = if paused {
        format!("Paused · {artist}")
    } else {
        format!("by {artist}")
    };
    let track_url =
        valid_video_id(&video_id).then(|| format!("https://music.youtube.com/watch?v={video_id}"));
    let mut presence = activity::Activity::new()
        .activity_type(activity::ActivityType::Listening)
        .details(details.clone())
        .state(discord_text(&status, "Listening on Yolite"));
    if let Some(url) = track_url.as_ref() {
        presence = presence.details_url(url.clone());
    }
    if thumbnail.starts_with("https://") || thumbnail.starts_with("http://") {
        let mut assets = activity::Assets::new()
            .large_image(thumbnail)
            .large_text(details);
        if let Some(url) = track_url {
            assets = assets.large_url(url);
        }
        presence = presence.assets(assets);
    }

    let mut discord = state
        .discord
        .lock()
        .map_err(|_| "Discord presence state is unavailable".to_string())?;
    if discord.is_none() {
        *discord = Some(connect_discord()?);
    }
    let result = discord
        .as_mut()
        .ok_or_else(|| "Discord is unavailable".to_string())?
        .set_activity(presence);
    if let Err(error) = result {
        *discord = None;
        return Err(format!("Could not update Discord presence: {error}"));
    }
    Ok(())
}

#[tauri::command]
fn clear_discord_presence(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut discord = state
        .discord
        .lock()
        .map_err(|_| "Discord presence state is unavailable".to_string())?;
    if let Some(client) = discord.as_mut() {
        client
            .clear_activity()
            .map_err(|error| format!("Could not clear Discord presence: {error}"))?;
    }
    Ok(())
}

pub fn run() {
    let state = Arc::new(AppState::default());
    let setup_state = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
        .setup(move |app| {
            match start_stream_server(app.handle().clone()) {
                Ok(base_url) => {
                    if let Ok(mut target) = setup_state.stream_base_url.lock() {
                        *target = Some(base_url);
                    }
                }
                Err(error) => eprintln!("Could not start Yolite stream server: {error}"),
            }
            match start_controller_server(app.handle().clone(), setup_state.clone()) {
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
            get_session,
            save_session,
            clear_session,
            import_browser_session,
            open_youtube_music,
            open_app_login,
            import_app_login_session,
            play_track_native,
            stop_native_playback,
            get_native_playback,
            set_native_volume,
            set_native_equalizer,
            set_native_pause,
            seek_native_playback,
            search_tracks,
            get_home,
            get_library,
            get_playlist,
            get_mix,
            prefetch_track,
            set_track_liked,
            create_playlist,
            add_track_to_playlist,
            resolve_track,
            update_discord_presence,
            clear_discord_presence,
            set_global_shortcuts,
            set_mini_player,
            load_plugins,
            update_remote_state,
            get_remote_controller
        ])
        .run(tauri::generate_context!())
        .expect("error while running Yolite");
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_external_command_clears_appimage_python_env() {
        let command = external_command(Path::new("yt-dlp"));
        let envs = command
            .get_envs()
            .map(|(key, value)| (key.to_string_lossy().to_string(), value.is_none()))
            .collect::<std::collections::HashMap<_, _>>();

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
