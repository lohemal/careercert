//! 학교 로고 · 발급 당시 로고 보관본 — SQL 만. 바이트는 이 프로그램이 만든 PNG 다(원본 경로 없음).

use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::logo::{self, Logo};
use crate::error::{AppError, AppResult};

fn tampered() -> AppError {
    AppError::new("CERT_TAMPERED", "발급 기록이 손상되었거나 바뀌었습니다. 이 증명서는 출력할 수 없습니다.").detail("logo")
}

/// 지금 학교 로고
pub fn current(conn: &Connection) -> AppResult<Option<Logo>> {
    let row: Option<(Vec<u8>, String)> = conn
        .query_row("SELECT data, sha256 FROM school_logo WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    match row {
        None => Ok(None),
        Some((data, sha)) => {
            let l = logo::from_stored(data)?;
            if l.sha256 != sha {
                return Err(AppError::new("LOGO_DAMAGED", "저장된 학교 로고가 손상되었습니다. 로고를 다시 등록해 주세요."));
            }
            Ok(Some(l))
        }
    }
}

/// 지금 학교 로고의 등록 시각
pub fn current_updated_at(conn: &Connection) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT updated_at FROM school_logo WHERE id = 1", [], |r| r.get(0)).optional()?)
}

pub fn set_current(conn: &Connection, l: &Logo, now: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO school_logo (id, mime, data, sha256, width, height, updated_at) VALUES (1, 'image/png', ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (id) DO UPDATE SET data = ?1, sha256 = ?2, width = ?3, height = ?4, updated_at = ?5",
        params![l.png.as_slice(), l.sha256, l.width, l.height, now],
    )?;
    Ok(())
}

/// 지운 줄 수
pub fn remove_current(conn: &Connection) -> AppResult<usize> {
    Ok(conn.execute("DELETE FROM school_logo WHERE id = 1", [])?)
}

/// 발급 당시 로고 보관 (같은 지문이면 이미 있는 것을 쓴다)
pub fn keep_for_certificate(conn: &Connection, l: &Logo, now: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO certificate_logos (sha256, mime, data, width, height, created_at) VALUES (?1, 'image/png', ?2, ?3, ?4, ?5)
         ON CONFLICT (sha256) DO NOTHING",
        params![l.sha256, l.png.as_slice(), l.width, l.height, now],
    )?;
    Ok(())
}

/// 발급본의 로고 — 바이트의 지문이 기록과 같아야 한다(다르면 변조)
pub fn for_certificate(conn: &Connection, sha256: &str) -> AppResult<Logo> {
    let data: Option<Vec<u8>> = conn
        .query_row("SELECT data FROM certificate_logos WHERE sha256 = ?1", [sha256], |r| r.get(0))
        .optional()?;
    let data = data.ok_or_else(tampered)?;
    let l = logo::from_stored(data).map_err(|_| tampered())?;
    if l.sha256 != sha256 {
        return Err(tampered());
    }
    Ok(l)
}
