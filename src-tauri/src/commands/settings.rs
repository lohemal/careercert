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
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let saved = state.db.write(|c| repo::settings::save(c, input, &now))?;
    Ok(view(saved.settings, saved.issuer_filled))
}
