use std::collections::HashMap;
use std::sync::Arc;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::models::*;

#[tauri::command]
pub(crate) fn set_global_shortcuts(
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
pub(crate) fn set_mini_player(
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
