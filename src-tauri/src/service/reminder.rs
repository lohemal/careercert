//! 이동용 백업 권장 알림 — 대시보드·설정의 조용한 안내에만 쓴다(팝업 없음).
//!
//! 마지막으로 **성공한** 이동용 백업의 시각을 `app_meta` 에 남긴다. 저장 위치·파일 이름은 남기지 않는다.
//! "그 뒤 자료가 바뀌었는가" 는 두 가지로 본다.
//!   * 변경 기록(audit_log)의 마지막 번호 — 강사·경력 등록/수정/종료/보관, 발급, 취소, 엑셀 가져오기,
//!     일괄 종료, 복구 비밀번호 설정·변경이 모두 여기에 남는다
//!   * 학교 설정의 마지막 저장 시각 — 설정 저장은 변경 기록을 남기지 않으므로 따로 본다

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::date;
use crate::error::AppResult;

const LAST_AT: &str = "portable_backup_at";
const LAST_AUDIT: &str = "portable_backup_audit_id";
const LAST_SETTINGS: &str = "portable_backup_settings_at";
pub const STALE_DAYS: i64 = 30;

fn meta(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM app_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute("INSERT OR REPLACE INTO app_meta (key, value) VALUES (?1, ?2)", params![key, value])?;
    Ok(())
}

fn audit_max(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COALESCE(MAX(id), 0) FROM audit_log", [], |r| r.get(0))?)
}

fn settings_at(conn: &Connection) -> AppResult<String> {
    Ok(conn
        .query_row("SELECT COALESCE(updated_at, '') FROM school_settings LIMIT 1", [], |r| r.get(0))
        .optional()?
        .unwrap_or_default())
}

/// 이동용 백업을 만들고 **확인까지 성공한 뒤**에 부른다(변경 기록을 남긴 다음 — 그 기록까지 포함한 지점).
pub fn record_export(conn: &Connection, now: &str) -> AppResult<()> {
    set_meta(conn, LAST_AT, now)?;
    set_meta(conn, LAST_AUDIT, &audit_max(conn)?.to_string())?;
    set_meta(conn, LAST_SETTINGS, &settings_at(conn)?)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// 보존할 자료가 없다 — 알리지 않는다
    NoData,
    /// 자료가 있는데 이동용 백업을 한 번도 만들지 않았다
    Never,
    /// 마지막 이동용 백업 뒤에 자료가 바뀌었다
    Changed,
    /// 마지막 이동용 백업 뒤 바뀐 것이 없다
    Current,
}

impl State {
    pub fn code(self) -> &'static str {
        match self {
            State::NoData => "NO_DATA",
            State::Never => "NEVER",
            State::Changed => "CHANGED",
            State::Current => "CURRENT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reminder {
    pub state: State,
    /// 마지막 이동용 백업 시각 `YYYY-MM-DDTHH:MM:SS`
    pub last_at: Option<String>,
    /// 마지막 이동용 백업 뒤 30일 이상 (자료가 있을 때만)
    pub stale: bool,
    /// 화면에 그대로 보일 안내 (비어 있으면 알리지 않는다)
    pub messages: Vec<String>,
}

pub fn reminder(conn: &Connection, today: NaiveDate) -> AppResult<Reminder> {
    let has_data: i64 = conn.query_row(
        "SELECT (SELECT count(*) FROM instructors) + (SELECT count(*) FROM careers) + (SELECT count(*) FROM certificates)",
        [],
        |r| r.get(0),
    )?;
    let last_at = meta(conn, LAST_AT)?;
    if has_data == 0 {
        return Ok(Reminder { state: State::NoData, last_at, stale: false, messages: vec![] });
    }
    let Some(at) = last_at.clone() else {
        return Ok(Reminder {
            state: State::Never,
            last_at: None,
            stale: false,
            messages: vec!["이동용 백업을 아직 만들지 않았습니다. 설정 → 데이터 관리에서 이동용 백업을 만들어 PC 밖 안전한 곳에 보관해 주세요.".into()],
        });
    };
    let recorded_audit: i64 = meta(conn, LAST_AUDIT)?.and_then(|v| v.parse().ok()).unwrap_or(0);
    let recorded_settings = meta(conn, LAST_SETTINGS)?.unwrap_or_default();
    let changed = audit_max(conn)? > recorded_audit || settings_at(conn)? > recorded_settings;
    let stale = date::today_of(&at).map(|d| (today - d).num_days() >= STALE_DAYS).unwrap_or(false);
    let mut messages = Vec::new();
    if changed {
        messages.push("최근 이동용 백업 이후 자료가 변경되었습니다.".into());
    }
    if stale {
        messages.push(format!("마지막 이동용 백업 후 {STALE_DAYS}일 이상 지났습니다."));
    }
    Ok(Reminder { state: if changed { State::Changed } else { State::Current }, last_at, stale, messages })
}

#[cfg(test)]
#[path = "reminder_tests.rs"]
mod reminder_tests;
