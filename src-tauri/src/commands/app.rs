//! 프로그램 자체의 정보.

use serde::Serialize;
use tauri::State;

use crate::db::migrate;
use crate::error::AppResult;
use crate::AppState;

/// 프로그램을 켤 때 있었던 일. **개인정보를 담지 않는다.**
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupNote {
    /// INTEGRITY · BACKUP_FAILED
    pub kind: String,
    /// success · warn · error · info
    pub tone: String,
    pub message: String,
}

impl StartupNote {
    pub fn new(kind: &str, tone: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            tone: tone.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub app_version: String,
    pub schema_version: i32,
    pub latest_schema_version: i32,
    pub db_path: String,
    pub backup_dir: String,
    pub sandbox: bool,
    pub notes: Vec<StartupNote>,
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<AppInfo> {
    Ok(AppInfo {
        app_version: app.package_info().version.to_string(),
        schema_version: state.db.schema_version()?,
        latest_schema_version: migrate::latest_version(),
        db_path: state.db.path().display().to_string(),
        backup_dir: state.db.backup_dir().display().to_string(),
        sandbox: state.sandbox,
        notes: state.notes(),
    })
}
