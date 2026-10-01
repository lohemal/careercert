//! 강사 등록·수정·보관·찾기. 바꾸는 일은 모두 변경 기록(audit_log)을 같은 트랜잭션에 남긴다.

use rusqlite::Connection;

use super::archive_error;
use crate::domain::audit::{changed_summary, Action};
use crate::domain::instructor::{changed_labels, prepare, Instructor, InstructorInput};
use crate::error::AppResult;
use crate::repo::audit;
use crate::repo::instructor::{self as repo, Filter, Summary};

const WHAT: &str = "강사";

pub fn create(conn: &Connection, input: InstructorInput, now: &str) -> AppResult<Instructor> {
    let f = prepare(input)?;
    let id = repo::insert(conn, &f, now)?;
    audit::add(conn, now, Action::InstructorCreate, Some(id), "등록")?;
    repo::get(conn, id)
}

pub fn update(conn: &Connection, id: i64, input: InstructorInput, now: &str) -> AppResult<Instructor> {
    let current = repo::get(conn, id)?;
    if current.is_archived() {
        return Err(archive_error::archived(WHAT));
    }
    let f = prepare(input)?;
    let changed = changed_labels(&current, &f);
    if changed.is_empty() {
        return Ok(current); // 바뀐 것이 없으면 수정 시각도 기록도 남기지 않는다
    }
    repo::update(conn, id, &f, now)?;
    audit::add(conn, now, Action::InstructorUpdate, Some(id), &changed_summary(&changed))?;
    repo::get(conn, id)
}

/// 보관. 경력은 그대로 둔다 — 강사를 다시 꺼내면 경력도 그대로 보인다.
pub fn archive(conn: &Connection, id: i64, now: &str) -> AppResult<Instructor> {
    if repo::get(conn, id)?.is_archived() {
        return Err(archive_error::already(WHAT));
    }
    repo::set_archived(conn, id, Some(now), now)?;
    audit::add(conn, now, Action::InstructorArchive, Some(id), "보관")?;
    repo::get(conn, id)
}

pub fn unarchive(conn: &Connection, id: i64, now: &str) -> AppResult<Instructor> {
    if !repo::get(conn, id)?.is_archived() {
        return Err(archive_error::not_archived(WHAT));
    }
    repo::set_archived(conn, id, None, now)?;
    audit::add(conn, now, Action::InstructorUnarchive, Some(id), "보관 해제")?;
    repo::get(conn, id)
}

/// 보관 여부와 상관없이 한 명.
pub fn get(conn: &Connection, id: i64) -> AppResult<Instructor> {
    repo::get(conn, id)
}

/// 찾기. 동명이인은 모두 나온다 — 구분은 `distinguisher` 와 경력으로 사람이 한다.
pub fn search(conn: &Connection, filter: &Filter) -> AppResult<Vec<Summary>> {
    repo::search(conn, filter)
}

#[cfg(test)]
#[path = "instructor_tests.rs"]
mod instructor_tests;
