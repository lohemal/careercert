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
    /// WebView2 입력 자동완성 저장이 꺼졌는가. None = 아직 확인 전
    pub autofill_off: Option<bool>,
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
        autofill_off: *state.autofill_off.lock().unwrap_or_else(|e| e.into_inner()),
    })
}

/// 업데이트 설치 직전 — 자료 연결을 닫는다(WAL 을 비우고 파일을 놓음). 이 뒤의 자료 요청은 거절된다.
/// 설치 프로그램이 곧 앱을 끝내고 새 버전을 켠다. 새 버전 첫 실행에서 구조가 바뀌면 마이그레이션 직전 백업이 먼저 뜬다.
#[tauri::command]
pub fn app_prepare_update(state: State<'_, AppState>) -> AppResult<()> {
    state.db.release()
}
