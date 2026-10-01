//! 복원 적용 — **열린 자료 파일을 바꿔치기하지 않는다.**
//!
//! ```text
//!   [복원하고 다시 시작]  ① 지금 자료를 backups/before_restore_*.db 로 백업 (SQLite 백업 API, 확인까지)
//!                        ② 준비된 메모리 DB 를 careercert.db.restore-pending 으로 쓰고 다시 열어 확인
//!                        ③ restore-pending.json 에 기대값(파일 SHA-256 · 건수) 기록
//!                        ④ 연결을 놓고 프로그램을 다시 시작
//!   [다음 시작, 연결을 열기 전]  apply_pending
//!                        ⑤ 대기 파일 확인(SHA-256 · integrity · app_id · 구조 버전)
//!                        ⑥ 지금 자료(.db·-wal)를 *.replaced 로 비켜 두고 대기 파일을 자료 이름으로
//!                        ⑦ 열어서 검증: integrity · 건수 · 모든 키를 이 PC DPAPI 로 풂 · 모든 발급본 복호화·지문
//!                        ⑧ 성공 → *.replaced 지움 / 실패 → 복원본 지우고 *.replaced 를 되돌림(지금 자료 유지)
//! ```
//! 어느 단계에서 멈춰도 지금 자료가 남는다 — ①의 백업, ⑥의 비켜 둔 파일.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::crypto::keys;
use crate::db::{backup, copy_with_backup_api, Db};
use crate::domain::audit::Action;
use crate::error::{AppError, AppResult};
use crate::repo::audit;
use crate::service::portable::{self, Counts, Prepared};

fn sibling(db_path: &Path, suffix: &str) -> PathBuf {
    let mut s = db_path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

pub fn pending_path(db_path: &Path) -> PathBuf {
    sibling(db_path, ".restore-pending")
}

pub fn marker_path(db_path: &Path) -> PathBuf {
    db_path.with_file_name("restore-pending.json")
}

/// 다음 시작에서 맞춰 볼 기대값 (개인정보 없음)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub pending_sha256: String,
    pub counts: Counts,
    pub verified_certificates: usize,
    pub backup_created_at: String,
    pub staged_at: String,
}

fn file_sha256(path: &Path) -> AppResult<String> {
    Ok(crate::crypto::plain_sha256(&std::fs::read(path)?))
}

/// 복원 대기 파일을 쓴다(②③). 부르기 전에 `before_restore_` 백업을 만들어 둘 것.
pub fn stage(prepared: &Prepared, db_path: &Path, now: &str) -> AppResult<Marker> {
    discard(db_path);
    let pending = pending_path(db_path);
    let part = portable::part_path(&pending);
    let _ = std::fs::remove_file(&part);
    let written = copy_with_backup_api(&prepared.conn, &part).and_then(|_| Ok(std::fs::rename(&part, &pending)?));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let check = backup::check(&pending);
    if !check.ok {
        discard(db_path);
        return Err(AppError::new("RESTORE_STAGE_FAILED", "복원 파일을 준비하지 못했습니다.")
            .detail(check.problem.unwrap_or_default()));
    }
    let marker = Marker {
        pending_sha256: file_sha256(&pending)?,
        counts: prepared.counts,
        verified_certificates: prepared.verified_certificates,
        backup_created_at: prepared.header.created_at.clone(),
        staged_at: now.into(),
    };
    if let Err(e) = portable::write_atomic(&marker_path(db_path), &serde_json::to_vec(&marker)?) {
        discard(db_path);
        return Err(e);
    }
    Ok(marker)
}

/// 대기 중인 복원을 버린다
pub fn discard(db_path: &Path) {
    let _ = std::fs::remove_file(pending_path(db_path));
    let _ = std::fs::remove_file(portable::part_path(&pending_path(db_path)));
    let _ = std::fs::remove_file(marker_path(db_path));
}

/// 시작할 때 복원을 적용한 결과
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// 대기 중인 복원 없음
    Nothing,
    Done { counts: Counts, verified_certificates: usize, backup_created_at: String },
    /// 적용하지 않았다 — 지금 자료 그대로
    Failed { message: String, retry_later: bool },
}

/// 잠깐 잠긴 파일(이전 프로그램이 아직 끝나는 중)을 기다리며 이름을 바꾼다
fn rename_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut last = None;
    for _ in 0..40 {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }
    }
    Err(last.expect("한 번은 시도했다"))
}

fn fail(message: &str, retry_later: bool) -> Applied {
    Applied::Failed { message: message.into(), retry_later }
}

/// 연결을 열기 **전에** 부른다(⑤~⑧).
pub fn apply_pending(db_path: &Path, now: &str) -> Applied {
    let pending = pending_path(db_path);
    if !pending.exists() {
        let _ = std::fs::remove_file(marker_path(db_path));
        return Applied::Nothing;
    }
    // ⑤ 대기 파일 확인
    let marker: Option<Marker> = std::fs::read(marker_path(db_path)).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let Some(marker) = marker else {
        discard(db_path);
        return fail("복원 정보가 없어 복원을 적용하지 않았습니다. 지금 자료를 그대로 씁니다.", false);
    };
    if file_sha256(&pending).ok().as_deref() != Some(marker.pending_sha256.as_str()) || !backup::check(&pending).ok {
        discard(db_path);
        return fail("복원 파일이 준비 뒤에 바뀌었거나 손상되어 적용하지 않았습니다. 지금 자료를 그대로 씁니다.", false);
    }

    // ⑥ 지금 자료를 비켜 둔다
    let wal = sibling(db_path, "-wal");
    let shm = sibling(db_path, "-shm");
    let old = sibling(db_path, ".replaced");
    let old_wal = sibling(db_path, ".replaced-wal");
    let _ = std::fs::remove_file(&old);
    let _ = std::fs::remove_file(&old_wal);
    let had_db = db_path.exists();
    if had_db && rename_retry(db_path, &old).is_err() {
        return fail("자료 파일이 사용 중이라 복원을 적용하지 못했습니다. 프로그램을 완전히 닫고 다시 열면 적용합니다.", true);
    }
    let had_wal = wal.exists();
    if had_wal && std::fs::rename(&wal, &old_wal).is_err() {
        let _ = std::fs::rename(&old, db_path);
        return fail("자료 파일이 사용 중이라 복원을 적용하지 못했습니다. 프로그램을 완전히 닫고 다시 열면 적용합니다.", true);
    }
    let _ = std::fs::remove_file(&shm);
    if std::fs::rename(&pending, db_path).is_err() {
        roll_back(db_path, had_db, had_wal);
        return fail("복원 파일을 자료 자리로 옮기지 못했습니다. 지금 자료를 그대로 씁니다.", true);
    }

    // ⑦ 열어서 검증
    match verify_restored(db_path, &marker, now) {
        Ok(()) => {
            let _ = std::fs::remove_file(&old);
            let _ = std::fs::remove_file(&old_wal);
            let _ = std::fs::remove_file(marker_path(db_path));
            Applied::Done {
                counts: marker.counts,
                verified_certificates: marker.verified_certificates,
                backup_created_at: marker.backup_created_at,
            }
        }
        Err(e) => {
            // ⑧ 실패 — 복원본을 지우고 지금 자료를 되돌린다
            let _ = std::fs::remove_file(db_path);
            let _ = std::fs::remove_file(&wal);
            let _ = std::fs::remove_file(&shm);
            roll_back(db_path, had_db, had_wal);
            let _ = std::fs::remove_file(marker_path(db_path));
            fail(&format!("복원한 자료를 확인하지 못해 되돌렸습니다. 지금 자료를 그대로 씁니다. ({})", e.user_message), false)
        }
    }
}

fn roll_back(db_path: &Path, had_db: bool, had_wal: bool) {
    if had_db {
        let _ = std::fs::rename(sibling(db_path, ".replaced"), db_path);
    }
    if had_wal {
        let _ = std::fs::rename(sibling(db_path, ".replaced-wal"), sibling(db_path, "-wal"));
    }
}

fn verify_restored(db_path: &Path, marker: &Marker, now: &str) -> AppResult<()> {
    let db = Db::open(db_path)?;
    if db.integrity()? != "ok" {
        return Err(AppError::new("RESTORE_VERIFY_FAILED", "무결성 검사에 실패했습니다."));
    }
    db.write(|c| {
        if portable::counts(c)? != marker.counts {
            return Err(AppError::new("RESTORE_VERIFY_FAILED", "자료 건수가 준비할 때와 다릅니다."));
        }
        for row in keys::all(c)? {
            keys::open_dpapi(&row)?; // 이 PC·이 Windows 사용자로 풀려야 한다
        }
        let n = portable::verify_certificates(c)?;
        if n != marker.verified_certificates {
            return Err(AppError::new("RESTORE_VERIFY_FAILED", "발급 기록 수가 준비할 때와 다릅니다."));
        }
        let k = marker.counts;
        audit::add(
            c,
            now,
            Action::Restore,
            None,
            &format!(
                "이동용 백업에서 복원 · 강사 {} · 경력 {} · 발급 {} · 취소 {} · 발급본 검증 {}건",
                k.instructors, k.careers, k.certificates_issued, k.certificates_voided, n
            ),
        )
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "restore_tests.rs"]
mod restore_tests;
