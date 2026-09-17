use std::fs;
use std::process::Command;
use std::sync::Arc;

use crate::{
    config::*,
    models::*,
    playback::{external_command, ytdlp_path},
};
use reqwest::{Client, header};
use serde_json::{Map, Value, json};
use tauri::Manager;

#[derive(Debug, Clone)]
pub(crate) struct NativeMusicClient {
    pub(crate) client: Client,
    pub(crate) config: Map<String, Value>,
    pub(crate) cookies: String,
    pub(crate) auth_user: u8,
    pub(crate) user_id: String,
    pub(crate) channel_id: String,
}

impl NativeMusicClient {
    pub(crate) async fn init(
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

    pub(crate) async fn request(&self, endpoint: &str, body: Value) -> Result<Value, String> {
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

    pub(crate) async fn account_profile(&self) -> Option<(String, String)> {
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

    pub(crate) async fn search_tracks(&self, query: &str) -> Result<Vec<TrackPayload>, String> {
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

    pub(crate) async fn liked_tracks(&self) -> Result<Vec<TrackPayload>, String> {
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

    pub(crate) async fn home_sections(&self) -> Result<Vec<LibrarySectionPayload>, String> {
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

    pub(crate) async fn library_landing(&self) -> Result<Value, String> {
        self.request("browse", json!({ "browseId": "FEmusic_library_landing" }))
            .await
    }

    pub(crate) async fn library_playlists(&self, landing_data: &Value) -> Vec<PlaylistPayload> {
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

    pub(crate) async fn playlist_tracks(
        &self,
        playlist_id: &str,
    ) -> Result<Vec<TrackPayload>, String> {
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

    pub(crate) async fn mix_tracks(
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

    pub(crate) async fn set_track_liked(&self, video_id: &str, liked: bool) -> Result<(), String> {
        let endpoint = if liked {
            "like/like"
        } else {
            "like/removelike"
        };
        self.request(endpoint, json!({ "target": { "videoId": video_id } }))
            .await
            .map(|_| ())
    }

    pub(crate) async fn create_playlist(&self, title: &str) -> Result<PlaylistPayload, String> {
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

    pub(crate) async fn add_track_to_playlist(
        &self,
        playlist_id: &str,
        video_id: &str,
    ) -> Result<(), String> {
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

pub(crate) async fn get_client(state: &AppState) -> Result<NativeMusicClient, String> {
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

pub(crate) fn extract_yt_config(html: &str) -> Map<String, Value> {
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

pub(crate) fn extract_json_object(source: &str, start: usize) -> Option<(String, usize)> {
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

pub(crate) fn config_str(config: &Map<String, Value>, key: &'static str) -> Result<String, String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing config key: {key}"))
}

pub(crate) fn merge_json(target: &mut Value, source: Value) {
    match (target, source) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in b {
                merge_json(a.entry(key).or_insert(Value::Null), value);
            }
        }
        (target, source) => *target = source,
    }
}

pub(crate) fn values_for_key<'a>(
    data: &'a Value,
    key: &str,
    dead_end: bool,
    out: &mut Vec<&'a Value>,
) {
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

pub(crate) fn traverse_list<'a>(data: &'a Value, keys: &[&str]) -> Vec<&'a Value> {
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

pub(crate) fn traverse_string(data: &Value, keys: &[&str]) -> String {
    traverse_list(data, keys)
        .into_iter()
        .find_map(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_default()
}

pub(crate) fn is_artist(value: &Value) -> bool {
    matches!(
        traverse_string(value, &["pageType"]).as_str(),
        "MUSIC_PAGE_TYPE_USER_CHANNEL" | "MUSIC_PAGE_TYPE_ARTIST"
    )
}

pub(crate) fn is_album(value: &Value) -> bool {
    traverse_string(value, &["pageType"]) == "MUSIC_PAGE_TYPE_ALBUM"
}

pub(crate) fn is_duration(value: &Value) -> bool {
    let text = value.get("text").and_then(Value::as_str).unwrap_or("");
    !text.is_empty()
        && text.chars().all(|ch| ch.is_ascii_digit() || ch == ':')
        && text.contains(':')
}

pub(crate) fn is_title(value: &Value) -> bool {
    traverse_string(value, &["musicVideoType"]).starts_with("MUSIC_VIDEO_TYPE_")
}

pub(crate) fn parse_duration(value: Option<&str>) -> u64 {
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

pub(crate) fn collect_thumbnail_items(data: &Value, out: &mut Vec<(u64, String)>) {
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

pub(crate) fn best_thumbnail(data: &Value) -> String {
    let mut items = Vec::new();
    collect_thumbnail_items(data, &mut items);
    items
        .into_iter()
        .max_by_key(|(width, _)| *width)
        .map(|(_, url)| url)
        .unwrap_or_default()
}

pub(crate) fn first_text(data: &Value, keys: &[&str]) -> String {
    traverse_string(data, keys)
}

pub(crate) fn parse_track_item(item: &Value, kind: &str) -> Option<TrackPayload> {
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

pub(crate) fn parse_playlist_panel_track_item(item: &Value) -> Option<TrackPayload> {
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

pub(crate) fn parse_track_items(data: &Value, kind: &str) -> Vec<TrackPayload> {
    traverse_list(data, &["musicResponsiveListItemRenderer"])
        .into_iter()
        .filter_map(|item| parse_track_item(item, kind))
        .collect()
}

pub(crate) fn subtitle_artist(item: &Value) -> String {
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

pub(crate) fn parse_two_row_track_item(item: &Value) -> Option<TrackPayload> {
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

pub(crate) fn parse_home_track_items(data: &Value) -> Vec<TrackPayload> {
    let mut tracks = parse_track_items(data, "SONG");
    tracks.extend(
        traverse_list(data, &["musicTwoRowItemRenderer"])
            .into_iter()
            .filter_map(parse_two_row_track_item),
    );
    tracks
}

pub(crate) fn parse_playlist_item(item: &Value) -> Option<PlaylistPayload> {
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

pub(crate) fn parse_playlist_items(data: &Value) -> Vec<PlaylistPayload> {
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

pub(crate) fn is_supported_playlist_id(id: &str) -> bool {
    matches!(id, "LM" | "SE")
        || id.starts_with("PL")
        || id.starts_with("RD")
        || id.starts_with("OL")
}

pub(crate) fn carousel_title(carousel: &Value) -> String {
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

pub(crate) fn parse_public_channel_playlists(data: &Value) -> Vec<PlaylistPayload> {
    traverse_list(data, &["musicCarouselShelfRenderer"])
        .into_iter()
        .filter(|carousel| carousel_title(carousel).eq_ignore_ascii_case("playlists"))
        .flat_map(parse_playlist_items)
        .collect()
}

pub(crate) fn public_channel_playlist_params(data: &Value) -> Vec<String> {
    pub(crate) fn walk(value: &Value, out: &mut Vec<String>) {
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

pub(crate) fn continuation_token(data: &Value) -> Option<String> {
    Some(traverse_string(data, &["continuationCommand", "token"]))
        .filter(|token| !token.is_empty())
        .or_else(|| {
            Some(traverse_string(data, &["continuation"])).filter(|token| !token.is_empty())
        })
}

pub(crate) fn dedupe_tracks(mut tracks: Vec<TrackPayload>, limit: usize) -> Vec<TrackPayload> {
    let mut seen = std::collections::HashSet::new();
    tracks.retain(|track| !track.id.is_empty() && seen.insert(track.id.clone()));
    tracks.truncate(limit);
    tracks
}

pub(crate) fn dedupe_mix_tracks(mut tracks: Vec<TrackPayload>, limit: usize) -> Vec<TrackPayload> {
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

pub(crate) fn dedupe_playlists(
    mut playlists: Vec<PlaylistPayload>,
    limit: usize,
) -> Vec<PlaylistPayload> {
    let mut seen = std::collections::HashSet::new();
    playlists.retain(|playlist| !playlist.id.is_empty() && seen.insert(playlist.id.clone()));
    playlists.truncate(limit);
    playlists
}

pub(crate) fn library_payload_text(data: &Value) -> Result<String, String> {
    serde_json::to_string(data).map_err(|error| error.to_string())
}

pub(crate) fn library_payload_has_content(data: &Value, raw: &str) -> bool {
    !parse_playlist_items(data).is_empty()
        || !parse_track_items(data, "SONG").is_empty()
        || raw.contains("musicTwoRowItemRenderer")
        || raw.contains("musicResponsiveListItemRenderer")
}

pub(crate) fn library_payload_is_authenticated(data: &Value) -> Result<bool, String> {
    let raw = library_payload_text(data)?;
    if library_payload_has_content(data, &raw) {
        return Ok(true);
    }
    Ok(!raw.contains("Sign in to access"))
}

#[tauri::command]
pub(crate) async fn get_session(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SessionPayload, String> {
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

pub(crate) fn normalize_user_id(user_id: &str) -> String {
    user_id.trim().trim_matches('/').to_string()
}

pub(crate) fn validate_user_id(user_id: &str) -> Result<String, String> {
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

pub(crate) fn normalize_channel_id(channel_id: &str) -> String {
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

pub(crate) fn validate_channel_id(channel_id: &str) -> Result<String, String> {
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

pub(crate) fn collect_user_ids_after_marker(source: &str, marker: &str, out: &mut Vec<String>) {
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

pub(crate) fn extract_user_ids_from_account_page(raw: &str) -> Vec<String> {
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

pub(crate) async fn detect_user_ids_from_account_page(cookies: &str) -> Vec<String> {
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

pub(crate) async fn resolve_user_id(
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

pub(crate) fn playlist_hint_score(playlists: &[PlaylistPayload]) -> usize {
    let titles = playlists
        .iter()
        .map(|playlist| playlist.title.to_lowercase())
        .collect::<std::collections::HashSet<_>>();
    MUSIC_ACCOUNT_HINT_PLAYLISTS
        .iter()
        .filter(|title| titles.contains(&title.to_lowercase()))
        .count()
}

pub(crate) async fn authenticated_auth_user(cookies: &str, user_id: &str) -> Option<u8> {
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
pub(crate) async fn save_session(
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
pub(crate) async fn clear_session(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SessionPayload, String> {
    write_config(&Config::default())?;
    *state.client.lock().await = None;
    get_session(state).await
}

#[tauri::command]
pub(crate) async fn search_tracks(
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
pub(crate) async fn get_library(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<LibraryPayload, String> {
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
pub(crate) async fn get_playlist(
    playlist_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<PlaylistDetailPayload, String> {
    let client = get_client(&state).await?;
    Ok(PlaylistDetailPayload {
        tracks: client.playlist_tracks(&playlist_id).await?,
    })
}

#[tauri::command]
pub(crate) async fn get_mix(
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
pub(crate) async fn get_home(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<LibraryPayload, String> {
    let client = get_client(&state).await?;
    Ok(LibraryPayload {
        needs_login: false,
        sections: client.home_sections().await.unwrap_or_default(),
        playlists: vec![],
    })
}

#[tauri::command]
pub(crate) async fn set_track_liked(
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
pub(crate) async fn create_playlist(
    title: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<PlaylistPayload, String> {
    let client = get_client(&state).await?;
    client.create_playlist(&title).await
}

#[tauri::command]
pub(crate) async fn add_track_to_playlist(
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

pub(crate) fn browser_cookie_sources() -> [&'static str; 7] {
    [
        "firefox", "chrome", "chromium", "brave", "edge", "opera", "vivaldi",
    ]
}

pub(crate) const MUSIC_COOKIE_IMPORT_PROBE_URL: &str =
    "https://music.youtube.com/watch?v=jNQXAC9IVRw";

pub(crate) fn validate_browser_source(browser: &str) -> Result<(), String> {
    if browser_cookie_sources().contains(&browser) {
        Ok(())
    } else {
        Err("Choose a supported browser cookie source".to_string())
    }
}

pub(crate) fn import_cookies_from_browser(
    app: &tauri::AppHandle,
    browser: &str,
) -> Result<String, String> {
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
pub(crate) fn open_youtube_music() -> Result<(), String> {
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
pub(crate) async fn open_app_login(app: tauri::AppHandle) -> Result<(), String> {
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
pub(crate) async fn import_app_login_session(
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
pub(crate) async fn import_browser_session(
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

pub(crate) fn valid_video_id(video_id: &str) -> bool {
    video_id.len() >= 6
        && video_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

pub(crate) fn normalize_playlist_id(playlist_id: &str) -> Result<String, String> {
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

pub(crate) fn valid_playlist_params(params: &str) -> bool {
    params
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '%')
}

pub(crate) fn validate_playlist_title(title: &str) -> Result<&str, String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 80 {
        return Err("Playlist name must be 1-80 characters".to_string());
    }
    Ok(title)
}
