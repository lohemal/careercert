//! 복구 비밀번호 설정·변경·확인.
//!
//! * **설정**(처음 한 번): 이 PC 의 DPAPI 사본으로 데이터 키를 풀어 새 비밀번호로 감싼다. 아직 키가 없으면
//!   (발급 전) 이때 키를 만든다. 이미 설정되어 있으면 거절한다 — 바꾸려면 **변경**으로.
//! * **변경**: 기존 비밀번호로 recovery_blob 을 **실제로 풀어야** 한다. 이 PC 의 DPAPI 로 풀 수 있다는 이유로
//!   기존 비밀번호 확인을 건너뛰지 않는다. 틀리면 아무것도 바꾸지 않는다(모든 키를 먼저 풀어 본 뒤에 쓴다).
//! * 감싼 뒤에는 새 blob 을 다시 풀어 같은 키가 나오는지 확인하고 나서 저장한다.
//! * 데이터 키는 그대로 — 증명서 암호문은 다시 암호화하지 않는다.
//! * 변경 기록에는 "설정"·"변경" 과 키 수만 적는다.

use rusqlite::Connection;

use crate::crypto::recovery::{self, KdfParams, Password};
use crate::crypto::{keys, DataKey};
use crate::domain::audit::Action;
use crate::error::{AppError, AppResult};
use crate::repo::audit;

fn not_set() -> AppError {
    AppError::new("RECOVERY_NOT_SET", "복구 비밀번호가 아직 설정되지 않았습니다. 설정 → 데이터 관리에서 먼저 설정해 주세요.")
}

/// 새 blob 을 만들고, 다시 풀어 같은 키인지 확인한다.
fn wrap_checked(key: &DataKey, password: &Password) -> AppResult<Vec<u8>> {
    let blob = recovery::wrap(key, password, KdfParams::current())?;
    let back = recovery::unwrap(&blob, password, &key.key_id)?;
    if back.expose() != key.expose() {
        return Err(AppError::internal("복구 사본 확인에 실패했습니다."));
    }
    Ok(blob)
}

/// 처음 설정. `Db::write` 안에서 부른다.
pub fn set(conn: &Connection, password: &Password, confirm: &Password, now: &str) -> AppResult<()> {
    recovery::validate_new(password, confirm)?;
    let mut rows = keys::all(conn)?;
    if rows.iter().any(|r| r.recovery_blob.is_some()) {
        return Err(AppError::new(
            "RECOVERY_ALREADY_SET",
            "복구 비밀번호가 이미 설정되어 있습니다. 바꾸려면 [복구 비밀번호 변경] 을 쓰세요.",
        ));
    }
    if rows.is_empty() {
        keys::current_or_create(conn, now)?; // 아직 발급 전 — 키를 지금 만든다
        rows = keys::all(conn)?;
    }
    for row in &rows {
        let key = keys::open_dpapi(row)?;
        let blob = wrap_checked(&key, password)?;
        keys::set_recovery(conn, &row.key_id, &blob, now)?;
    }
    audit::add(conn, now, Action::RecoverySet, None, &format!("복구 비밀번호 설정 · 키 {}개", rows.len()))?;
    Ok(())
}

/// 변경 — 기존 비밀번호가 맞아야 한다. `Db::write` 안에서 부른다.
pub fn change(conn: &Connection, old: &Password, new: &Password, confirm: &Password, now: &str) -> AppResult<()> {
    recovery::validate_new(new, confirm)?;
    // 1) 기존 비밀번호로 모든 키를 먼저 푼다 — 하나라도 틀리면 아무것도 쓰지 않는다
    let opened = open_all(conn, old)?;
    // 2) 새 비밀번호로 감싸고 다시 풀어 확인한 뒤 바꾼다
    let mut blobs = Vec::with_capacity(opened.len());
    for key in &opened {
        blobs.push(wrap_checked(key, new)?);
    }
    for (key, blob) in opened.iter().zip(&blobs) {
        keys::set_recovery(conn, &key.key_id, blob, now)?;
    }
    audit::add(conn, now, Action::RecoveryChange, None, &format!("복구 비밀번호 변경 · 키 {}개", opened.len()))?;
    Ok(())
}

/// 비밀번호로 모든 키를 푼다. 설정되지 않은 키가 있으면 `RECOVERY_NOT_SET`, 틀리면 `RECOVERY_PASSWORD_WRONG`.
/// 이 PC 의 DPAPI 사본이 풀리면 같은 키인지도 맞춰 본다.
pub fn open_all(conn: &Connection, password: &Password) -> AppResult<Vec<DataKey>> {
    let rows = keys::all(conn)?;
    if rows.is_empty() || rows.iter().any(|r| r.recovery_blob.is_none()) {
        return Err(not_set());
    }
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let key = recovery::unwrap(row.recovery_blob.as_deref().unwrap_or_default(), password, &row.key_id)?;
        if let Ok(local) = keys::open_dpapi(row) {
            if local.expose() != key.expose() {
                return Err(AppError::new("RECOVERY_MISMATCH", "복구 사본과 이 PC 의 키가 서로 다릅니다.")
                    .detail(row.key_id.clone()));
            }
        }
        out.push(key);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod recovery_tests;
