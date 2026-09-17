use std::sync::Arc;

use qrcode::{QrCode, render::svg};

use crate::models::*;

#[tauri::command]
pub(crate) fn update_remote_state(
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
pub(crate) fn get_remote_controller(
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
