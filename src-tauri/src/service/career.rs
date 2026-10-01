//! 경력 등록·수정·종료·보관. 바꾸는 일은 모두 변경 기록(audit_log)을 같은 트랜잭션에 남긴다.
//!
//! 미래 종료일: 저장은 하되 `confirm_future` 로 확인을 받아야 한다(`domain::career::check_future`).
//! "오늘" 은 `now` 에서 뽑는다 — 시험이 시각을 고정하면 오늘도 고정된다.

use rusqlite::Connection;

use super::archive_error;
use crate::domain::audit::{changed_summary, Action};
use crate::domain::career::{self, Career, CareerInput, EndReason, Term};
use crate::domain::date;
use crate::error::{AppError, AppResult};
use crate::repo::{audit, career as repo, instructor as instructor_repo};

const WHAT: &str = "경력";
const OWNER: &str = "강사의 경력";

pub(crate) fn today(now: &str) -> AppResult<chrono::NaiveDate> {
    date::today_of(now).map_err(|_| AppError::internal(format!("시각 형식이 아닙니다: {now}")))
}

/// 경력을 고칠 수 있는 상태인가 — 경력도, 그 강사도 보관되지 않아야 한다.
pub(crate) fn editable(conn: &Connection, id: i64) -> AppResult<Career> {
    let c = repo::get(conn, id)?;
    if c.is_archived() {
        return Err(archive_error::archived(WHAT));
    }
    if instructor_repo::get(conn, c.instructor_id)?.is_archived() {
        return Err(archive_error::archived(OWNER));
    }
    Ok(c)
}

/// 종료 기록에 적는 말 `종료일 2026-10-31 · 계약만료`.
pub(crate) fn end_summary(term: &Term) -> String {
    match term {
        Term::Active => "재직중".into(),
        Term::Ended { end_date, reason } => format!("종료일 {} · {}", date::to_iso(*end_date), reason.label()),
    }
}

pub fn create(
    conn: &Connection,
    instructor_id: i64,
    input: CareerInput,
    confirm_future: bool,
    now: &str,
) -> AppResult<Career> {
    if instructor_repo::get(conn, instructor_id)?.is_archived() {
        return Err(archive_error::archived(OWNER));
    }
    let f = career::prepare(input)?;
    career::check_future(&f.term, today(now)?, confirm_future)?;
    let id = repo::insert(conn, instructor_id, &f, None, now)?;
    audit::add(conn, now, Action::CareerCreate, Some(id), "등록")?;
    repo::get(conn, id)
}

/// 내용 전체 수정. 상태도 바꿀 수 있다(잘못 종료한 것을 재직중으로 되돌리기, 종료일 고치기) —
/// 검사는 새로 만들 때와 같은 `prepare` 를 지난다. 별도의 '종료 취소' 기능은 두지 않는다.
pub fn update(conn: &Connection, id: i64, input: CareerInput, confirm_future: bool, now: &str) -> AppResult<Career> {
    let current = editable(conn, id)?;
    let f = career::prepare(input)?;
    let changed = career::changed_labels(&current.fields, &f);
    if changed.is_empty() {
        return Ok(current);
    }
    // 종료일을 그대로 두고 다른 칸만 고칠 때는 다시 묻지 않는다
    if current.fields.term.end_date() != f.term.end_date() {
        career::check_future(&f.term, today(now)?, confirm_future)?;
    }
    repo::update(conn, id, &f, now)?;
    audit::add(conn, now, Action::CareerUpdate, Some(id), &changed_summary(&changed))?;
    repo::get(conn, id)
}

/// 재직중 경력을 종료한다. 이미 종료된 경력이면 `CAREER_ALREADY_ENDED`.
pub fn end(
    conn: &Connection,
    id: i64,
    end_date: &str,
    reason: EndReason,
    confirm_future: bool,
    now: &str,
) -> AppResult<Career> {
    let c = editable(conn, id)?;
    let term = career::end(&c.fields, end_date, reason)?;
    career::check_future(&term, today(now)?, confirm_future)?;
    repo::set_term(conn, id, &term, now)?;
    audit::add(conn, now, Action::CareerEnd, Some(id), &end_summary(&term))?;
    repo::get(conn, id)
}

pub fn archive(conn: &Connection, id: i64, now: &str) -> AppResult<Career> {
    if repo::get(conn, id)?.is_archived() {
        return Err(archive_error::already(WHAT));
    }
    repo::set_archived(conn, id, Some(now), now)?;
    audit::add(conn, now, Action::CareerArchive, Some(id), "보관")?;
    repo::get(conn, id)
}

pub fn unarchive(conn: &Connection, id: i64, now: &str) -> AppResult<Career> {
    if !repo::get(conn, id)?.is_archived() {
        return Err(archive_error::not_archived(WHAT));
    }
    repo::set_archived(conn, id, None, now)?;
    audit::add(conn, now, Action::CareerUnarchive, Some(id), "보관 해제")?;
    repo::get(conn, id)
}

#[cfg_attr(not(test), allow(dead_code))] // Phase 4 증명서 작성이 쓴다
pub fn get(conn: &Connection, id: i64) -> AppResult<Career> {
    repo::get(conn, id)
}

/// 한 강사의 경력 (시작일 차례). 강사가 없으면 NOT_FOUND.
pub fn list(conn: &Connection, instructor_id: i64, include_archived: bool) -> AppResult<Vec<Career>> {
    instructor_repo::get(conn, instructor_id)?;
    repo::list_by_instructor(conn, instructor_id, include_archived)
}

#[cfg(test)]
#[path = "career_tests.rs"]
mod career_tests;
