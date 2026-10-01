//! 발급이력 — 목록 · "이 내용으로 새 증명서 작성" 의 출발점 · 대시보드 요약 · 복구 설정 상태.
//!
//! 어느 명령도 복호화하지 않는다(`service::history` 는 암호문 칸을 읽지 않는다).
//! 상세는 `commands::issuance::certificate_issued`, 재출력·취소도 `commands::issuance` 의 명령을 그대로 쓴다.

use serde::Serialize;
use tauri::State;

use super::now;
use crate::domain::date;
use crate::error::AppResult;
use crate::repo::certificate::Meta;
use crate::service::career::today;
use crate::service::history::{self as service, Filter};
use crate::AppState;

/// 목록 한 줄 — 주민번호·주소는 없다(가린 주민번호도 상세에서만).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRow {
    pub id: i64,
    pub issue_no: String,
    /// YYYY-MM-DD
    pub issued_on: String,
    pub issued_on_label: String,
    pub holder_name: String,
    pub purpose: String,
    pub item_count: i64,
    /// FULL · MASK_BACK
    pub rrn_display: String,
    /// ISSUED · VOIDED
    pub status: String,
    pub voided_at: Option<String>,
    pub created_at: String,
}

fn row(m: Meta) -> HistoryRow {
    HistoryRow {
        issued_on_label: date::parse_iso(&m.issued_on).map(date::display).unwrap_or_else(|_| m.issued_on.clone()),
        id: m.id,
        issue_no: m.issue_no,
        issued_on: m.issued_on,
        holder_name: m.holder_name,
        purpose: m.purpose,
        item_count: m.item_count,
        rrn_display: m.rrn_display,
        status: m.status,
        voided_at: m.voided_at,
        created_at: m.created_at,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub rows: Vec<HistoryRow>,
    pub truncated: bool,
    pub limit: i64,
}

#[tauri::command]
pub fn certificate_history(state: State<'_, AppState>, filter: Filter) -> AppResult<HistoryPage> {
    let page = state.db.read(|c| service::search(c, &filter))?;
    Ok(HistoryPage { rows: page.rows.into_iter().map(row).collect(), truncated: page.truncated, limit: service::LIST_LIMIT })
}

/// "이 내용으로 새 증명서 작성" 의 출발점 — 주민번호·주소·발급번호는 들어 있지 않다.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyPlanView {
    pub from_id: i64,
    pub from_issue_no: String,
    pub from_status: String,
    pub instructor_id: Option<i64>,
    pub issued_on: String,
    pub purpose: String,
    pub mask_rrn: bool,
    pub selected_career_ids: Vec<i64>,
    pub excluded_career_ids: Vec<i64>,
    pub original_count: usize,
    pub unlinked_count: usize,
    pub notices: Vec<String>,
}

#[tauri::command]
pub fn certificate_copy_plan(state: State<'_, AppState>, id: i64) -> AppResult<CopyPlanView> {
    let today = date::to_iso(today(&now())?);
    let p = state.db.read(|c| service::copy_plan(c, id, &today))?;
    Ok(CopyPlanView {
        mask_rrn: p.rrn_display == "MASK_BACK",
        from_id: p.from_id,
        from_issue_no: p.from_issue_no,
        from_status: p.from_status,
        instructor_id: p.instructor_id,
        issued_on: p.issued_on,
        purpose: p.purpose,
        selected_career_ids: p.selected_career_ids,
        excluded_career_ids: p.excluded_career_ids,
        original_count: p.original_count,
        unlinked_count: p.unlinked_count,
        notices: p.notices,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentView {
    pub issued_count: i64,
    pub voided_count: i64,
    pub issued: Vec<HistoryRow>,
    pub voided: Vec<HistoryRow>,
}

/// 대시보드 — 최근 발급 · 최근 취소 (각 5건)
#[tauri::command]
pub fn certificate_recent(state: State<'_, AppState>) -> AppResult<RecentView> {
    let r = state.db.read(|c| service::recent(c, 5))?;
    Ok(RecentView {
        issued_count: r.issued_count,
        voided_count: r.voided_count,
        issued: r.issued.into_iter().map(row).collect(),
        voided: r.voided.into_iter().map(row).collect(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryView {
    /// NO_DATA · NOT_SET · READY
    pub state: &'static str,
    pub ready: bool,
    pub message: &'static str,
    pub certificates: i64,
}

/// 암호화 자료의 이동용 복구 설정 상태 (키 값은 보이지 않는다)
#[tauri::command]
pub fn recovery_status(state: State<'_, AppState>) -> AppResult<RecoveryView> {
    let r = state.db.read(service::recovery)?;
    Ok(RecoveryView {
        state: r.state.code(),
        ready: r.state == service::RecoveryState::Ready,
        message: r.state.message(),
        certificates: r.certificates,
    })
}
