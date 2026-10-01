//! 경력 — 화면에 보내는 모양과 명령.
//!
//! 기간·상태·사유를 **글자로 만들어 보낸다** (`period`, `statusLabel` …). 화면은 날짜 표시 규칙을
//! 갖지 않는다. 날짜 칸을 채우기 위한 `startDate`·`endDate`(YYYY-MM-DD)만 따로 보낸다.

use chrono::NaiveDate;
use serde::Serialize;
use tauri::State;

use super::now;
use crate::domain::career::{suggest_duty, Career, CareerInput, EndReason, DEFAULT_POSITION};
use crate::domain::date;
use crate::error::AppResult;
use crate::service::career as service;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerView {
    pub id: i64,
    pub instructor_id: i64,
    pub program_name: String,
    pub position: String,
    pub duty: String,
    /// 입력 칸용 `YYYY-MM-DD`
    pub start_date: String,
    pub end_date: Option<String>,
    /// ACTIVE · ENDED
    pub status: &'static str,
    /// 재직중 · 종료
    pub status_label: &'static str,
    /// CONTRACT_END · TERMINATED
    pub end_reason: Option<&'static str>,
    /// 계약만료 · 중도해지
    pub end_reason_label: Option<&'static str>,
    /// `2026.03.04 ~ 현재` — domain 이 만든 표시
    pub period: String,
    /// `2026.03.04` — 기간의 시작 쪽 (좁은 화면에서 줄을 나눌 때)
    pub period_from: String,
    /// `현재` / `2027.02.12`
    pub period_to: String,
    /// 종료일이 오늘보다 뒤
    pub future_end: bool,
    /// 입력 칸용 `YYYY-MM-DD` — 예정 종료일 (관리용, 증명서 기간과 무관)
    pub planned_end_date: Option<String>,
    /// `2027.02.05`
    pub planned_end_label: Option<String>,
    /// 재직중인데 예정 종료일이 지났다 (알리기만 한다)
    pub planned_end_passed: bool,
    pub memo: String,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub updated_at: String,
}

pub(crate) fn view(c: Career, today: NaiveDate) -> CareerView {
    let term = c.fields.term;
    let (period_from, period_to) = c.period_parts();
    CareerView {
        period: c.period(),
        period_from,
        period_to,
        archived: c.is_archived(),
        future_end: crate::domain::career::future_end(&term, today).is_some(),
        planned_end_date: c.fields.planned_end_date.map(date::to_iso),
        planned_end_label: c.fields.planned_end_date.map(date::display),
        planned_end_passed: crate::domain::career::planned_end_passed(&c.fields, today),
        id: c.id,
        instructor_id: c.instructor_id,
        start_date: date::to_iso(c.fields.start_date),
        end_date: term.end_date().map(date::to_iso),
        status: term.status().code(),
        status_label: term.status().label(),
        end_reason: term.reason().map(|r| r.code()),
        end_reason_label: term.reason().map(|r| r.label()),
        program_name: c.fields.program_name,
        position: c.fields.position,
        duty: c.fields.duty,
        memo: c.fields.memo,
        archived_at: c.archived_at,
        updated_at: c.updated_at,
    }
}

fn today(now: &str) -> AppResult<NaiveDate> {
    service::today(now)
}

#[tauri::command]
pub fn career_list(state: State<'_, AppState>, instructor_id: i64, include_archived: bool) -> AppResult<Vec<CareerView>> {
    let now = now();
    let t = today(&now)?;
    let list = state.db.read(|c| service::list(c, instructor_id, include_archived))?;
    Ok(list.into_iter().map(|c| view(c, t)).collect())
}

#[tauri::command]
pub fn career_create(
    state: State<'_, AppState>,
    instructor_id: i64,
    input: CareerInput,
    confirm_future: bool,
) -> AppResult<CareerView> {
    let now = now();
    let c = state.db.write(|c| service::create(c, instructor_id, input, confirm_future, &now))?;
    Ok(view(c, today(&now)?))
}

#[tauri::command]
pub fn career_update(
    state: State<'_, AppState>,
    id: i64,
    input: CareerInput,
    confirm_future: bool,
) -> AppResult<CareerView> {
    let now = now();
    let c = state.db.write(|c| service::update(c, id, input, confirm_future, &now))?;
    Ok(view(c, today(&now)?))
}

#[tauri::command]
pub fn career_end(
    state: State<'_, AppState>,
    id: i64,
    end_date: String,
    reason: EndReason,
    confirm_future: bool,
) -> AppResult<CareerView> {
    let now = now();
    let c = state.db.write(|c| service::end(c, id, &end_date, reason, confirm_future, &now))?;
    Ok(view(c, today(&now)?))
}

#[tauri::command]
pub fn career_archive(state: State<'_, AppState>, id: i64) -> AppResult<CareerView> {
    let now = now();
    let c = state.db.write(|c| service::archive(c, id, &now))?;
    Ok(view(c, today(&now)?))
}

#[tauri::command]
pub fn career_unarchive(state: State<'_, AppState>, id: i64) -> AppResult<CareerView> {
    let now = now();
    let c = state.db.write(|c| service::unarchive(c, id, &now))?;
    Ok(view(c, today(&now)?))
}

/// 새 경력 칸의 기본값과 지도사항 제안. 제안을 **적용하는 것은 사용자**다.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerHints {
    pub default_position: &'static str,
    /// `방과후학교 {프로그램명}` — 프로그램명이 비면 None
    pub duty_suggestion: Option<String>,
    /// 오늘 `YYYY-MM-DD` — 날짜 칸 기본값
    pub today: String,
}

#[tauri::command]
pub fn career_hints(program_name: String) -> AppResult<CareerHints> {
    let now = now();
    Ok(CareerHints {
        default_position: DEFAULT_POSITION,
        duty_suggestion: suggest_duty(&program_name),
        today: date::to_iso(today(&now)?),
    })
}
