//! 재직중 경력 일괄 종료 — 후보 · 미리보기 · 적용.
//!
//! 적용 순서: **검사(트랜잭션 밖) → 백업(`before_bulk_end_`) → 트랜잭션 안에서 다시 미리보기·검사 → 쓰기.**
//! 어차피 거절될 작업으로 백업을 쌓지 않으려고 백업 전에 한 번 검사하고, 백업과 쓰기 사이에
//! 자료가 바뀌었을 수 있으므로 트랜잭션 안에서 다시 검사한다.

use serde::Serialize;
use tauri::State;

use super::career::{view, CareerView};
use super::now;
use crate::db::backup::{self, Kind};
use crate::domain::date;
use crate::error::AppResult;
use crate::service::bulk_end::{self as service, BulkEndRequest};
use crate::service::career::today;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateView {
    #[serde(flatten)]
    pub career: CareerView,
    pub instructor_name: String,
    pub distinguisher: String,
}

#[tauri::command]
pub fn bulk_end_candidates(state: State<'_, AppState>) -> AppResult<Vec<CandidateView>> {
    let now = now();
    let t = today(&now)?;
    let list = state.db.read(service::candidates)?;
    Ok(list
        .into_iter()
        .map(|c| CandidateView {
            career: view(c.career, t),
            instructor_name: c.instructor_name,
            distinguisher: c.distinguisher,
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItemView {
    pub career_id: i64,
    pub instructor_id: Option<i64>,
    pub instructor_name: String,
    pub distinguisher: String,
    pub program_name: String,
    pub before: String,
    pub after: Option<String>,
    pub problem: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewView {
    pub items: Vec<PreviewItemView>,
    /// `2027.02.12`
    pub end_date_label: String,
    pub reason_label: &'static str,
    pub future: bool,
    pub ok: bool,
    pub problems: usize,
    pub state_key: String,
}

#[tauri::command]
pub fn bulk_end_preview(state: State<'_, AppState>, request: BulkEndRequest) -> AppResult<PreviewView> {
    let now = now();
    let p = state.db.read(|c| service::preview(c, &request, &now))?;
    Ok(PreviewView {
        end_date_label: date::display(p.end_date),
        reason_label: p.reason.label(),
        future: p.future,
        ok: p.ok,
        problems: p.problems(),
        state_key: p.state_key,
        items: p
            .items
            .into_iter()
            .map(|i| PreviewItemView {
                career_id: i.career_id,
                instructor_id: i.instructor_id,
                instructor_name: i.instructor_name,
                distinguisher: i.distinguisher,
                program_name: i.program_name,
                before: i.before,
                after: i.after,
                problem: i.problem,
            })
            .collect(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedView {
    pub ended: usize,
    /// 적용 직전에 뜬 백업 파일 이름
    pub backup_file: String,
}

#[tauri::command]
pub fn bulk_end_apply(
    state: State<'_, AppState>,
    request: BulkEndRequest,
    state_key: String,
    confirm_future: bool,
) -> AppResult<AppliedView> {
    let now = now();
    // 1) 어차피 거절될 작업이면 백업을 뜨지 않는다
    state.db.read(|c| service::check(c, &request, &state_key, confirm_future, &now))?;
    // 2) 바꾸기 직전 자료를 떠 둔다
    let saved = backup::create(&state.db, Kind::BeforeBulkEnd, chrono::Local::now())?;
    // 3) 트랜잭션 안에서 다시 확인하고 전부 또는 하나도
    let done = state
        .db
        .write(|c| service::apply(c, &request, &state_key, confirm_future, &now))?;
    Ok(AppliedView {
        ended: done.ended,
        backup_file: saved.file_name,
    })
}
