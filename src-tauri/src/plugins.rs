use std::fs;
use std::path::PathBuf;

use crate::{config::*, models::*};

pub(crate) fn plugin_root() -> Result<PathBuf, String> {
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
pub(crate) fn load_plugins() -> Result<Vec<PluginPayload>, String> {
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
