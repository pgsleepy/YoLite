use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::Config;
use ring::digest::{SHA1_FOR_LEGACY_USE_ONLY, digest};

pub(crate) fn config_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir().ok_or_else(|| "Could not find config directory".to_string())?;
    Ok(base.join("yolite").join("config.json"))
}

pub(crate) fn read_config() -> Config {
    let Ok(path) = config_path() else {
        return Config::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Config::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub(crate) fn write_config(config: &Config) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let body = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(path, body).map_err(|error| error.to_string())
}

pub(crate) const MAX_COOKIE_HEADER_BYTES: usize = 8 * 1024;
pub(crate) const MUSIC_ORIGIN: &str = "https://music.youtube.com";
pub(crate) const MUSIC_ACCOUNT_HINT_PLAYLISTS: &[&str] = &[
    "Liked Music",
    "2025 Recap",
    "Jordy playlist",
    "Dancing",
    "Liked songs (Spotify)",
    "ROCK",
    "SPACE KEES",
];
pub(crate) const YTDLP_AUDIO_FORMAT: &str =
    "bestaudio[ext=m4a]/bestaudio[protocol^=http]/bestaudio/best[protocol^=http]/best";
pub(crate) const STREAM_CACHE_TTL_MS: u64 = 45 * 60 * 1000;
pub(crate) static COOKIE_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn is_youtube_cookie_domain(domain: &str) -> bool {
    let domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    domain == "youtube.com" || domain.ends_with(".youtube.com")
}

pub(crate) fn is_supported_cookie_domain(domain: &str) -> bool {
    let domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    is_youtube_cookie_domain(&domain) || domain == "google.com" || domain.ends_with(".google.com")
}

pub(crate) fn is_likely_youtube_cookie_name(name: &str) -> bool {
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

pub(crate) fn is_music_auth_cookie_name(name: &str) -> bool {
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

pub(crate) fn cookie_pair(name: &str, value: &str) -> Option<String> {
    let name = name.trim();
    let value = value.trim();
    (!name.is_empty() && !value.is_empty()).then(|| format!("{name}={value}"))
}

pub(crate) fn cookie_name(pair: &str) -> &str {
    pair.split_once('=').map(|(name, _)| name).unwrap_or(pair)
}

pub(crate) fn cookie_value<'a>(cookies: &'a str, name: &str) -> Option<&'a str> {
    cookies
        .split(';')
        .map(str::trim)
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(cookie_name, value)| (cookie_name.trim() == name).then_some(value.trim()))
}

pub(crate) fn youtube_auth_header(cookies: &str) -> Option<String> {
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

pub(crate) fn join_cookie_pairs(pairs: Vec<String>) -> String {
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

pub(crate) fn compact_cookie_header(pairs: Vec<String>) -> String {
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

pub(crate) fn firefox_cookie_databases() -> Vec<PathBuf> {
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

pub(crate) fn import_firefox_cookies_direct() -> Result<String, String> {
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

pub(crate) struct TempCookieFile {
    pub(crate) path: PathBuf,
}

impl TempCookieFile {
    pub(crate) fn new(cookies: &str) -> Result<Option<Self>, String> {
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

pub(crate) fn normalize_cookies(input: &str) -> String {
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

pub(crate) fn normalize_webview_cookies(cookies: Vec<tauri::webview::Cookie<'static>>) -> String {
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
