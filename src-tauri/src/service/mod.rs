//! 업무 절차. "읽고 → domain 규칙으로 판단하고 → repo 로 쓴다".
//!
//! 함수는 `&Connection` 과 시각(`now`)을 받는다 — `Db::write` 한 번(=한 트랜잭션) 안에서
//! 부르고, 시험에서는 시각을 고정할 수 있다. Tauri 를 모르므로 서버로 옮겨도 그대로 쓴다.
//!
//! 보관 규칙 (강사·경력 공통)
//!   * 지우지 않고 보관한다. 기본 목록에서 빠지고 `include_archived` 로 다시 볼 수 있다.
//!   * **보관된 것은 고치지 않는다** — 먼저 보관을 해제한다. 보관된 강사에게 경력을 더하거나
//!     그 강사의 경력을 고치는 것도 막는다. 숨겨 둔 자료가 모르는 사이에 바뀌지 않게.
//!   * 이미 보관된 것을 다시 보관하거나, 보관되지 않은 것을 해제하면 오류로 알린다.

pub mod bulk_end;
pub mod career;
pub mod certificate;
pub mod dashboard;
pub mod history;
pub mod import;
pub mod issuance;
pub mod portable;
pub mod recovery;
pub mod reminder;
pub mod restore;
pub mod instructor;

/// 보관 상태가 맞지 않을 때의 오류들.
pub(crate) mod archive_error {
    use crate::error::AppError;

    pub fn archived(what: &str) -> AppError {
        AppError::new(
            "ARCHIVED",
            format!("보관된 {what}은(는) 고칠 수 없습니다. 먼저 보관을 해제해 주세요."),
        )
    }

    pub fn already(what: &str) -> AppError {
        AppError::new("ALREADY_ARCHIVED", format!("이미 보관된 {what}입니다."))
    }

    pub fn not_archived(what: &str) -> AppError {
        AppError::new("NOT_ARCHIVED", format!("보관되지 않은 {what}입니다."))
    }
}

#[cfg(test)]
#[path = "perf_tests.rs"]
mod perf_tests;
