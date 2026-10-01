//! `key_store` — DPAPI 로 감싼 데이터 키. 키 평문은 DB 에 없다.
//!
//! * 처음 발급할 때 키를 하나 만든다(그전에는 표가 비어 있다).
//! * 쓰는 키는 가장 최근 것 하나. 증명서마다 어느 키로 봉인했는지(`key_id`)를 함께 적는다 —
//!   나중에 키를 바꿔도(Phase 8 이후) 옛 증명서를 열 수 있다.

use rusqlite::{params, Connection, OptionalExtension};

use super::{dpapi, DataKey, KEY_LEN};
use crate::error::{AppError, AppResult};

pub const ALGORITHM: &str = "AES-256-GCM";

fn unwrap(key_id: String, blob: &[u8]) -> AppResult<DataKey> {
    let plain = dpapi::unprotect(blob)?;
    if plain.len() != KEY_LEN {
        return Err(AppError::new("KEY_UNAVAILABLE", "자료 암호 키가 손상되었습니다.").detail("length"));
    }
    let mut b = [0u8; KEY_LEN];
    b.copy_from_slice(&plain);
    Ok(DataKey::new(key_id, b))
}

/// 이 키 번호의 키를 푼다.
pub fn get(conn: &Connection, key_id: &str) -> AppResult<DataKey> {
    let blob: Option<Vec<u8>> = conn
        .query_row("SELECT dpapi_blob FROM key_store WHERE key_id = ?1", [key_id], |r| r.get(0))
        .optional()?;
    let blob = blob.ok_or_else(|| AppError::new("KEY_UNAVAILABLE", "자료 암호 키를 찾을 수 없습니다.").detail("missing key_id"))?;
    unwrap(key_id.to_string(), &blob)
}

/// 지금 쓰는 키. 없으면 만든다(쓰기 트랜잭션 안에서 부른다).
pub fn current_or_create(conn: &Connection, now: &str) -> AppResult<DataKey> {
    let row: Option<(String, Vec<u8>)> = conn
        .query_row(
            "SELECT key_id, dpapi_blob FROM key_store ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((id, blob)) = row {
        return unwrap(id, &blob);
    }
    let key = DataKey::generate(uuid::Uuid::new_v4().to_string());
    let blob = dpapi::protect(key.expose())?;
    conn.execute(
        "INSERT INTO key_store (key_id, algorithm, dpapi_blob, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![key.key_id, ALGORITHM, blob, now],
    )?;
    Ok(key)
}

/// 지금 쓰는 키(만들지 않는다). 아직 없으면 None.
#[cfg_attr(not(test), allow(dead_code))] // Phase 8 백업·복원이 쓴다
pub fn current(conn: &Connection) -> AppResult<Option<DataKey>> {
    let row: Option<(String, Vec<u8>)> = conn
        .query_row(
            "SELECT key_id, dpapi_blob FROM key_store ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    row.map(|(id, blob)| unwrap(id, &blob)).transpose()
}

/// (키 수, 복구 사본이 없는 키 수). 키 값은 읽지 않는다 — 복구 설정 상태 안내용.
pub fn recovery_counts(conn: &Connection) -> AppResult<(i64, i64)> {
    Ok(conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(recovery_blob IS NULL), 0) FROM key_store",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?)
}
