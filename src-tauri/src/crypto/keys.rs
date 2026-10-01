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

/// key_store 한 줄 (감싼 사본 그대로 — 풀지 않는다)
#[derive(Clone)]
pub struct KeyRow {
    pub key_id: String,
    pub dpapi_blob: Vec<u8>,
    pub recovery_blob: Option<Vec<u8>>,
}

impl std::fmt::Debug for KeyRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyRow")
            .field("key_id", &self.key_id)
            .field("recovery", &self.recovery_blob.is_some())
            .finish_non_exhaustive()
    }
}

/// 모든 키 (만든 차례)
pub fn all(conn: &Connection) -> AppResult<Vec<KeyRow>> {
    let mut stmt = conn.prepare("SELECT key_id, dpapi_blob, recovery_blob FROM key_store ORDER BY id")?;
    let rows = stmt
        .query_map([], |r| Ok(KeyRow { key_id: r.get(0)?, dpapi_blob: r.get(1)?, recovery_blob: r.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// DPAPI 사본을 푼다 (이 PC·이 Windows 사용자에서만 풀린다)
pub fn open_dpapi(row: &KeyRow) -> AppResult<DataKey> {
    unwrap(row.key_id.clone(), &row.dpapi_blob)
}

/// 복구 비밀번호로 감싼 사본을 바꾼다
pub fn set_recovery(conn: &Connection, key_id: &str, blob: &[u8], now: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE key_store SET recovery_blob = ?2, recovery_set_at = ?3 WHERE key_id = ?1",
        params![key_id, blob, now],
    )?;
    if n == 1 {
        Ok(())
    } else {
        Err(AppError::internal("key_store 행을 찾지 못했습니다."))
    }
}

/// 이 PC 의 DPAPI 로 다시 감싼다 (다른 PC·계정에서 복원할 때)
pub fn rewrap_dpapi(conn: &Connection, key: &DataKey) -> AppResult<()> {
    let blob = dpapi::protect(key.expose())?;
    let n = conn.execute("UPDATE key_store SET dpapi_blob = ?2 WHERE key_id = ?1", params![key.key_id, blob])?;
    if n == 1 {
        Ok(())
    } else {
        Err(AppError::internal("key_store 행을 찾지 못했습니다."))
    }
}
