//! 학교 기본정보.

use serde::Serialize;
use tauri::State;

use crate::domain::settings::{missing, FieldKey, SchoolSettings, SettingsInput};
use crate::error::AppResult;
use crate::repo;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingField {
    pub key: FieldKey,
    pub label: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub settings: SchoolSettings,
    /// 증명서에 찍히는데 비어 있는 칸. 비어 있으면 설정 완료.
    pub missing: Vec<MissingField>,
    /// 저장하면서 발급자 표기를 학교명으로 채웠는가 (저장 결과에서만 true 가 될 수 있다)
    pub issuer_filled: bool,
}

fn view(settings: SchoolSettings, issuer_filled: bool) -> SettingsView {
    let missing = missing(&settings)
        .into_iter()
        .map(|key| MissingField {
            key,
            label: key.label(),
        })
        .collect();
    SettingsView {
        settings,
        missing,
        issuer_filled,
    }
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> AppResult<SettingsView> {
    let s = state.db.read(repo::settings::get)?;
    Ok(view(s, false))
}

#[tauri::command]
pub fn settings_save(state: State<'_, AppState>, input: SettingsInput) -> AppResult<SettingsView> {
    let now = super::now();
    let saved = state.db.write(|c| repo::settings::save(c, input, &now))?;
    Ok(view(saved.settings, saved.issuer_filled))
}

// ---------------------------------------------------------------
// 학교 로고 (v0.1.2)
// ---------------------------------------------------------------

/// 설정 화면의 로고 칸 — 미리보기는 보관본(색 그대로)을 data URL 로. 경로는 없다.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoView {
    pub present: bool,
    pub preview: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub updated_at: Option<String>,
}

fn logo_view(conn: &rusqlite::Connection) -> AppResult<LogoView> {
    let l = repo::logo::current(conn)?;
    Ok(LogoView {
        present: l.is_some(),
        preview: l.as_ref().map(|l| crate::domain::logo::data_url(&l.png)),
        width: l.as_ref().map(|l| l.width),
        height: l.as_ref().map(|l| l.height),
        updated_at: repo::logo::current_updated_at(conn)?,
    })
}

#[tauri::command]
pub fn school_logo_get(state: State<'_, AppState>) -> AppResult<LogoView> {
    state.db.read(logo_view)
}

/// 파일 고르기 → 바이트 읽기 → 검사 → 보관. 고른 파일의 경로는 여기서만 쓰고 버린다(저장·기록하지 않음).
#[tauri::command]
pub async fn school_logo_pick(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<Option<LogoView>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("학교 로고 고르기")
        .add_filter("그림 (PNG · JPG · WebP)", &["png", "jpg", "jpeg", "webp"])
        .pick_file(move |p| {
            let _ = tx.send(p);
        });
    let Some(picked) = rx.await.map_err(|e| crate::error::AppError::internal(e.to_string()))? else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    let meta = std::fs::metadata(&path).map_err(|_| crate::error::AppError::new("LOGO_INVALID", "그림 파일을 읽지 못했습니다."))?;
    if meta.len() as usize > crate::domain::logo::MAX_FILE_BYTES {
        return Err(crate::error::AppError::new("LOGO_INVALID", "그림 파일이 너무 큽니다. 10MB 이하의 로고 파일을 골라 주세요."));
    }
    let bytes = std::fs::read(&path).map_err(|_| crate::error::AppError::new("LOGO_INVALID", "그림 파일을 읽지 못했습니다."))?;
    drop(path);
    let db = state.db.clone();
    let view = tauri::async_runtime::spawn_blocking(move || {
        let now = super::now();
        db.write(|c| {
            crate::service::school_logo::set(c, &bytes, &now)?;
            logo_view(c)
        })
    })
    .await
    .map_err(|e| crate::error::AppError::internal(e.to_string()))??;
    Ok(Some(view))
}

#[tauri::command]
pub fn school_logo_remove(state: State<'_, AppState>) -> AppResult<LogoView> {
    let now = super::now();
    state.db.write(|c| {
        crate::service::school_logo::remove(c, &now)?;
        logo_view(c)
    })
}
