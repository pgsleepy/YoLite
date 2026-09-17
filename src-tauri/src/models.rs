use std::path::PathBuf;

use discord_rich_presence::DiscordIpcClient;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;

use crate::youtube::NativeMusicClient;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default)]
    pub(crate) cookies: String,
    #[serde(default)]
    pub(crate) auth_user: u8,
    #[serde(default, alias = "userId")]
    pub(crate) user_id: String,
    #[serde(default, alias = "channelId")]
    pub(crate) channel_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionPayload {
    pub(crate) logged_in: bool,
    pub(crate) cookie_bytes: usize,
    pub(crate) config_path: String,
    pub(crate) auth_user: u8,
    pub(crate) user_id: String,
    pub(crate) channel_id: String,
    pub(crate) library_authenticated: Option<bool>,
    pub(crate) profile_name: String,
    pub(crate) profile_picture: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteTrack {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) thumbnail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemotePlaybackState {
    pub(crate) track: Option<RemoteTrack>,
    pub(crate) playing: bool,
    pub(crate) volume: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteControllerPayload {
    pub(crate) url: String,
    pub(crate) qr_svg: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginPayload {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) css: String,
    pub(crate) discover_categories: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginManifest {
    pub(crate) name: String,
    #[serde(default = "default_plugin_version")]
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) discover_categories: Vec<String>,
}

pub(crate) fn default_plugin_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackPayload {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) artist_id: Option<String>,
    pub(crate) album: String,
    pub(crate) duration: u64,
    pub(crate) thumbnail: String,
    pub(crate) url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) playlist_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) playlist_params: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SearchPayload {
    pub(crate) results: Vec<TrackPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibrarySectionPayload {
    pub(crate) title: String,
    pub(crate) tracks: Vec<TrackPayload>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) layout: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistPayload {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) thumbnail: String,
    pub(crate) url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryPayload {
    pub(crate) needs_login: bool,
    pub(crate) sections: Vec<LibrarySectionPayload>,
    pub(crate) playlists: Vec<PlaylistPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistDetailPayload {
    pub(crate) tracks: Vec<TrackPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolvePayload {
    pub(crate) stream_url: String,
    pub(crate) fallback_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackPayload {
    pub(crate) playing: bool,
    pub(crate) paused: bool,
    pub(crate) position: f64,
    pub(crate) duration: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VisualizerPayload {
    pub(crate) bass: f64,
    pub(crate) mids: f64,
    pub(crate) treble: f64,
    pub(crate) energy: f64,
    pub(crate) peak: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EqualizerPayload {
    #[serde(default)]
    pub(crate) preamp: f64,
    #[serde(default)]
    pub(crate) bass: f64,
    #[serde(default)]
    pub(crate) low_mid: f64,
    #[serde(default)]
    pub(crate) mid: f64,
    #[serde(default)]
    pub(crate) high_mid: f64,
    #[serde(default)]
    pub(crate) treble: f64,
    #[serde(default)]
    pub(crate) normalization: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PlaybackAudioSettings {
    pub(crate) equalizer: EqualizerPayload,
    pub(crate) visualizer_enabled: bool,
}

impl EqualizerPayload {
    pub(crate) fn db(value: f64) -> f64 {
        value.clamp(-12.0, 12.0)
    }

    pub(crate) fn is_flat(&self) -> bool {
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

    pub(crate) fn mpv_filter(&self) -> Option<String> {
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

pub(crate) struct NativePlayback {
    pub(crate) player: std::process::Child,
    pub(crate) ipc_path: PathBuf,
}

pub(crate) struct CachedStream {
    pub(crate) url: String,
    pub(crate) expires_at: u64,
}

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) client: AsyncMutex<Option<NativeMusicClient>>,
    pub(crate) stream_base_url: std::sync::Mutex<Option<String>>,
    pub(crate) stream_cache: std::sync::Mutex<std::collections::HashMap<String, CachedStream>>,
    pub(crate) playback: std::sync::Mutex<Option<NativePlayback>>,
    pub(crate) playback_audio: std::sync::Mutex<PlaybackAudioSettings>,
    pub(crate) discord: std::sync::Mutex<Option<DiscordIpcClient>>,
    pub(crate) remote_state: std::sync::Mutex<RemotePlaybackState>,
    pub(crate) controller_url: std::sync::Mutex<Option<String>>,
    pub(crate) main_window_size: std::sync::Mutex<Option<tauri::PhysicalSize<u32>>>,
}
