//! 강사 등록·수정·보관·찾기.

use rusqlite::Connection;

use super::archive_error;
use crate::domain::instructor::{prepare, Instructor, InstructorInput};
use crate::error::AppResult;
use crate::repo::instructor::{self as repo, Filter, Summary};

const WHAT: &str = "강사";

pub fn create(conn: &Connection, input: InstructorInput, now: &str) -> AppResult<Instructor> {
    let f = prepare(input)?;
    let id = repo::insert(conn, &f, now)?;
    repo::get(conn, id)
}

pub fn update(conn: &Connection, id: i64, input: InstructorInput, now: &str) -> AppResult<Instructor> {
    let current = repo::get(conn, id)?;
    if current.is_archived() {
        return Err(archive_error::archived(WHAT));
    }
    let f = prepare(input)?;
    repo::update(conn, id, &f, now)?;
    repo::get(conn, id)
}

/// 보관. 경력은 그대로 둔다 — 강사를 다시 꺼내면 경력도 그대로 보인다.
pub fn archive(conn: &Connection, id: i64, now: &str) -> AppResult<Instructor> {
    if repo::get(conn, id)?.is_archived() {
        return Err(archive_error::already(WHAT));
    }
    repo::set_archived(conn, id, Some(now), now)?;
    repo::get(conn, id)
}

pub fn unarchive(conn: &Connection, id: i64, now: &str) -> AppResult<Instructor> {
    if !repo::get(conn, id)?.is_archived() {
        return Err(archive_error::not_archived(WHAT));
    }
    repo::set_archived(conn, id, None, now)?;
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
