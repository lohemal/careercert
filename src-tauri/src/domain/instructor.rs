//! 강사 기본정보 규칙.
//!
//!   * 강사는 **이름으로 식별하지 않는다.** 이름은 찾기·표시용이고 식별은 id / uuid 다.
//!     동명이인이 있을 수 있으므로 같은 이름도 그대로 받는다.
//!   * 주민등록번호·주소는 여기에 **없다** (결정 D3). 발급할 때마다 입력해 증명서 기록에만 남긴다.
//!   * 입력은 앞뒤 공백만 정리한다.

use serde::Deserialize;

use crate::error::{AppError, AppResult};

pub const NAME_MAX: usize = 50;
pub const DISTINGUISHER_MAX: usize = 100;
pub const PHONE_MAX: usize = 50;
pub const MEMO_MAX: usize = 1000;

/// 화면에서 받은 값.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructorInput {
    pub name: String,
    /// 동명이인 구분 메모 (예: 1985년생, 마술 강사, 이전 계약자) — 선택
    pub distinguisher: String,
    pub phone: String,
    pub memo: String,
}

/// 검사를 마친 값. `prepare` 로만 만든다 — repo 는 이것만 받는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructorFields {
    pub name: String,
    pub distinguisher: String,
    pub phone: String,
    pub memo: String,
}

/// 저장된 강사.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instructor {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    pub distinguisher: String,
    pub phone: String,
    pub memo: String,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Instructor {
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }
}

pub fn prepare(raw: InstructorInput) -> AppResult<InstructorFields> {
    let v = InstructorFields {
        name: raw.name.trim().to_string(),
        distinguisher: raw.distinguisher.trim().to_string(),
        phone: raw.phone.trim().to_string(),
        memo: raw.memo.trim().to_string(),
    };
    if v.name.is_empty() {
        return Err(AppError::invalid("강사 이름을 입력해 주세요."));
    }
    for (label, value, max) in [
        ("이름", &v.name, NAME_MAX),
        ("구분 메모", &v.distinguisher, DISTINGUISHER_MAX),
        ("연락처", &v.phone, PHONE_MAX),
        ("메모", &v.memo, MEMO_MAX),
    ] {
        if value.chars().count() > max {
            return Err(AppError::invalid(format!("{label}은(는) {max}자 이내로 입력해 주세요.")));
        }
    }
    Ok(v)
}

#[cfg(test)]
#[path = "instructor_tests.rs"]
mod instructor_tests;
