//! `audit_log` — 덧붙이기와 읽기만. 고치거나 지우는 함수는 두지 않는다.

use rusqlite::{params, Connection};

use crate::domain::audit::Action;
use crate::error::AppResult;

pub fn add(conn: &Connection, at: &str, action: Action, target_id: Option<i64>, summary: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO audit_log (at, action, target_type, target_id, summary) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![at, action.code(), action.target_type(), target_id, summary],
    )?;
    Ok(())
}

/// 기록 한 줄.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub at: String,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<i64>,
    pub summary: String,
}

/// 한 대상의 기록 (오래된 것부터).
#[cfg_attr(not(test), allow(dead_code))] // 기록 보기 화면은 아직 없다 — 시험이 쓴다
pub fn for_target(conn: &Connection, target_type: &str, target_id: i64) -> AppResult<Vec<Entry>> {
    let mut stmt = conn.prepare(
        "SELECT at, action, target_type, target_id, summary FROM audit_log
          WHERE target_type = ?1 AND target_id = ?2 ORDER BY id",
    )?;
    let rows = stmt
        .query_map(params![target_type, target_id], |r| {
            Ok(Entry {
                at: r.get(0)?,
                action: r.get(1)?,
                target_type: r.get(2)?,
                target_id: r.get(3)?,
                summary: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 전체 기록 (오래된 것부터) — 시험에서 "값이 새지 않았는가" 를 본다.
#[cfg(test)]
pub fn all(conn: &Connection) -> AppResult<Vec<Entry>> {
    let mut stmt = conn.prepare("SELECT at, action, target_type, target_id, summary FROM audit_log ORDER BY id")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Entry {
                at: r.get(0)?,
                action: r.get(1)?,
                target_type: r.get(2)?,
                target_id: r.get(3)?,
                summary: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
