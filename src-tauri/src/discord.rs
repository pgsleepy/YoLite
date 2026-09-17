use std::sync::Arc;

use crate::{models::*, youtube::valid_video_id};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};

pub(crate) const DISCORD_APP_ID: &str = "1549900591088541827";

pub(crate) fn discord_text(value: &str, fallback: &str) -> String {
    let text = value.trim();
    let text = if text.is_empty() { fallback } else { text };
    text.chars().take(128).collect()
}

pub(crate) fn connect_discord() -> Result<DiscordIpcClient, String> {
    let mut client = DiscordIpcClient::new(DISCORD_APP_ID);
    client
        .connect()
        .map_err(|error| format!("Could not connect to Discord: {error}"))?;
    Ok(client)
}

#[tauri::command]
pub(crate) fn update_discord_presence(
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
pub(crate) fn clear_discord_presence(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
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
