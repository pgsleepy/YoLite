use serde::Serialize;
use tauri_plugin_updater::UpdaterExt;

const PACKAGE_MANAGER_UPDATE_MODE: &str = "package-manager";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateStatus {
    available: bool,
    current_version: String,
    version: Option<String>,
    notes: Option<String>,
    managed_externally: bool,
}

fn updates_managed_externally_value(value: Option<&str>) -> bool {
    value == Some(PACKAGE_MANAGER_UPDATE_MODE)
}

fn updates_managed_externally() -> bool {
    updates_managed_externally_value(std::env::var("YOLITE_UPDATE_MODE").ok().as_deref())
}

fn package_manager_status(app: &tauri::AppHandle) -> UpdateStatus {
    UpdateStatus {
        available: false,
        current_version: app.package_info().version.to_string(),
        version: None,
        notes: None,
        managed_externally: true,
    }
}

#[tauri::command]
pub(crate) async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateStatus, String> {
    if updates_managed_externally() {
        return Ok(package_manager_status(&app));
    }

    let current_version = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;

    Ok(match update {
        Some(update) => UpdateStatus {
            available: true,
            current_version,
            version: Some(update.version),
            notes: update.body,
            managed_externally: false,
        },
        None => UpdateStatus {
            available: false,
            current_version,
            version: None,
            notes: None,
            managed_externally: false,
        },
    })
}

#[tauri::command]
pub(crate) async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    if updates_managed_externally() {
        return Err("This installation is updated by the system package manager".to_string());
    }

    let update = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Yolite is already up to date".to_string())?;

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::updates_managed_externally_value;

    #[test]
    fn package_manager_mode_only_accepts_explicit_value() {
        assert!(updates_managed_externally_value(Some("package-manager")));
        assert!(!updates_managed_externally_value(Some("appimage")));
        assert!(!updates_managed_externally_value(None));
    }
}
