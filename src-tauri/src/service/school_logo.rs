//! 학교 로고 등록·삭제 (v0.1.2).
//!
//! * 앞으로 **새로 발급하는** 증명서에만 쓰인다. 이미 발급한 증명서는 발급 당시 로고(certificate_logos)를 쓴다.
//! * 원본 파일 경로는 받지도 남기지도 않는다 — 바이트만 받아 검사·보관한다.
//! * 변경 기록에는 "등록 · 가로×세로" / "삭제" 만 (이동용 백업 권장 알림이 이 기록으로 변경을 안다).

use rusqlite::Connection;

use crate::domain::audit::Action;
use crate::domain::logo::{self, Logo};
use crate::error::AppResult;
use crate::repo::{audit, logo as repo};

/// 파일 바이트를 검사해 학교 로고로 둔다. `Db::write` 안에서 부른다.
pub fn set(conn: &Connection, bytes: &[u8], now: &str) -> AppResult<Logo> {
    let l = logo::accept(bytes)?;
    repo::set_current(conn, &l, now)?;
    audit::add(conn, now, Action::SchoolLogo, None, &format!("학교 로고 등록 · {}×{}", l.width, l.height))?;
    Ok(l)
}

/// 학교 로고를 지운다. 없었으면 아무것도 하지 않는다.
pub fn remove(conn: &Connection, now: &str) -> AppResult<bool> {
    let n = repo::remove_current(conn)?;
    if n > 0 {
        audit::add(conn, now, Action::SchoolLogo, None, "학교 로고 삭제")?;
    }
    Ok(n > 0)
}

#[cfg(test)]
#[path = "school_logo_tests.rs"]
mod school_logo_tests;
