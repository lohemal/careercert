//! 대시보드 요약.

use serde::Serialize;
use tauri::State;

use super::now;
use crate::error::AppResult;
use crate::service::dashboard as service;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckItemView {
    pub instructor_id: i64,
    pub instructor_name: String,
    pub distinguisher: String,
    pub detail: String,
    /// 기간 겹침 항목만
    pub overlap: Option<OverlapView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlapView {
    pub career_a_id: i64,
    pub career_b_id: i64,
    pub fingerprint: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckView {
    pub kind: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub items: Vec<CheckItemView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewView {
    pub instructors: i64,
    pub active_careers: i64,
    pub checks: Vec<CheckView>,
}

#[tauri::command]
pub fn dashboard_overview(state: State<'_, AppState>) -> AppResult<OverviewView> {
    let now = now();
    let o = state.db.read(|c| service::overview(c, &now))?;
    Ok(OverviewView {
        instructors: o.instructors,
        active_careers: o.active_careers,
        checks: o
            .checks
            .into_iter()
            .map(|k| CheckView {
                kind: k.kind,
                title: k.title,
                description: k.description,
                items: k
                    .items
                    .into_iter()
                    .map(|i| CheckItemView {
                        instructor_id: i.instructor_id,
                        instructor_name: i.instructor_name,
                        distinguisher: i.distinguisher,
                        detail: i.detail,
                        overlap: i.overlap.map(|o| OverlapView {
                            career_a_id: o.career_a_id,
                            career_b_id: o.career_b_id,
                            fingerprint: o.fingerprint,
                        }),
                    })
                    .collect(),
            })
            .collect(),
    })
}
