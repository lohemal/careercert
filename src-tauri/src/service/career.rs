//! 경력 등록·수정·종료·보관.

use rusqlite::Connection;

use super::archive_error;
use crate::domain::career::{self, Career, CareerInput, EndReason};
use crate::error::AppResult;
use crate::repo::{career as repo, instructor as instructor_repo};

const WHAT: &str = "경력";
const OWNER: &str = "강사의 경력";

/// 경력을 고칠 수 있는 상태인가 — 경력도, 그 강사도 보관되지 않아야 한다.
fn editable(conn: &Connection, id: i64) -> AppResult<Career> {
    let c = repo::get(conn, id)?;
    if c.is_archived() {
        return Err(archive_error::archived(WHAT));
    }
    if instructor_repo::get(conn, c.instructor_id)?.is_archived() {
        return Err(archive_error::archived(OWNER));
    }
    Ok(c)
}

pub fn create(conn: &Connection, instructor_id: i64, input: CareerInput, now: &str) -> AppResult<Career> {
    if instructor_repo::get(conn, instructor_id)?.is_archived() {
        return Err(archive_error::archived(OWNER));
    }
    let f = career::prepare(input)?;
    let id = repo::insert(conn, instructor_id, &f, None, now)?;
    repo::get(conn, id)
}

/// 내용 전체 수정. 상태도 바꿀 수 있다(잘못 종료한 것을 재직중으로 되돌리기 등) —
/// 검사는 새로 만들 때와 같은 `prepare` 를 지난다.
pub fn update(conn: &Connection, id: i64, input: CareerInput, now: &str) -> AppResult<Career> {
    editable(conn, id)?;
    let f = career::prepare(input)?;
    repo::update(conn, id, &f, now)?;
    repo::get(conn, id)
}

/// 재직중 경력을 종료한다. 이미 종료된 경력이면 `CAREER_ALREADY_ENDED`.
pub fn end(conn: &Connection, id: i64, end_date: &str, reason: EndReason, now: &str) -> AppResult<Career> {
    let c = editable(conn, id)?;
    let term = career::end(&c.fields, end_date, reason)?;
    repo::set_term(conn, id, &term, now)?;
    repo::get(conn, id)
}

pub fn archive(conn: &Connection, id: i64, now: &str) -> AppResult<Career> {
    if repo::get(conn, id)?.is_archived() {
        return Err(archive_error::already(WHAT));
    }
    repo::set_archived(conn, id, Some(now), now)?;
    repo::get(conn, id)
}

pub fn unarchive(conn: &Connection, id: i64, now: &str) -> AppResult<Career> {
    if !repo::get(conn, id)?.is_archived() {
        return Err(archive_error::not_archived(WHAT));
    }
    repo::set_archived(conn, id, None, now)?;
    repo::get(conn, id)
}

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
