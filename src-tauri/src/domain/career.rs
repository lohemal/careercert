//! 경력 규칙.
//!
//! 경력의 상태는 둘뿐이고, 상태마다 갖는 값이 정해져 있다.
//!
//! ```text
//!   ACTIVE (재직중)  시작일 ~ (종료일 없음)   종료 사유 없음
//!   ENDED  (종료)    시작일 ~ 종료일         종료 사유 있음   종료일 ≥ 시작일
//! ```
//!
//! 이것을 `Term` 하나로 나타내므로 "재직중인데 종료일이 있다" 같은 값은 **만들 수조차 없다.**
//! DB(003 마이그레이션의 CHECK)도 같은 규칙을 한 번 더 막는다.
//!
//! "현재" 라는 글자는 저장하지 않는다. 재직중이면 종료일이 없을 뿐이고, `현재` 는 표시할 때
//! `domain::date::display_end` 가 붙인다.

use chrono::NaiveDate;
use serde::Deserialize;

use super::date::{self, DateError};
use crate::error::{AppError, AppResult};

pub const PROGRAM_MAX: usize = 100;
pub const POSITION_MAX: usize = 50;
pub const DUTY_MAX: usize = 200;
pub const MEMO_MAX: usize = 1000;

/// 직위 기본값 — 화면이 새 경력에 처음 채워 넣는다.
pub const DEFAULT_POSITION: &str = "강사";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CareerStatus {
    Active,
    Ended,
}

impl CareerStatus {
    pub fn code(self) -> &'static str {
        match self {
            CareerStatus::Active => "ACTIVE",
            CareerStatus::Ended => "ENDED",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CareerStatus::Active => "재직중",
            CareerStatus::Ended => "종료",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EndReason {
    /// 계약 기간이 끝나서
    ContractEnd,
    /// 중도에 계약을 해지해서
    Terminated,
}

impl EndReason {
    pub fn code(self) -> &'static str {
        match self {
            EndReason::ContractEnd => "CONTRACT_END",
            EndReason::Terminated => "TERMINATED",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EndReason::ContractEnd => "계약만료",
            EndReason::Terminated => "중도해지",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "CONTRACT_END" => Some(EndReason::ContractEnd),
            "TERMINATED" => Some(EndReason::Terminated),
            _ => None,
        }
    }
}

/// 경력의 끝 — 재직중이거나, 언제 왜 끝났거나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Term {
    Active,
    Ended { end_date: NaiveDate, reason: EndReason },
}

impl Term {
    pub fn status(&self) -> CareerStatus {
        match self {
            Term::Active => CareerStatus::Active,
            Term::Ended { .. } => CareerStatus::Ended,
        }
    }

    pub fn end_date(&self) -> Option<NaiveDate> {
        match self {
            Term::Active => None,
            Term::Ended { end_date, .. } => Some(*end_date),
        }
    }

    pub fn reason(&self) -> Option<EndReason> {
        match self {
            Term::Active => None,
            Term::Ended { reason, .. } => Some(*reason),
        }
    }

    /// DB 세 칸 (status, end_date, end_reason) 로 읽는다. 어긋난 조합은 None —
    /// DB CHECK 가 막으므로 정상이라면 나올 수 없다.
    pub fn from_columns(status: &str, end_date: Option<&str>, reason: Option<&str>) -> Option<Term> {
        match (status, end_date, reason) {
            ("ACTIVE", None, None) => Some(Term::Active),
            ("ENDED", Some(d), Some(r)) => Some(Term::Ended {
                end_date: date::parse_iso(d).ok()?,
                reason: EndReason::from_code(r)?,
            }),
            _ => None,
        }
    }
}

/// 화면에서 받은 값. 날짜는 `YYYY-MM-DD`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerInput {
    pub program_name: String,
    pub position: String,
    pub duty: String,
    pub start_date: String,
    pub status: CareerStatus,
    /// 재직중이면 비우거나 null
    pub end_date: Option<String>,
    /// 재직중이면 null
    pub end_reason: Option<EndReason>,
    /// 예정 종료일 (계약서 기준, 선택). 관리용 — 증명서 기간에는 쓰지 않는다.
    #[serde(default)]
    pub planned_end_date: Option<String>,
    pub memo: String,
}

/// 검사를 마친 값. `prepare` 로만 만든다 — repo 는 이것만 받는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CareerFields {
    pub program_name: String,
    pub position: String,
    pub duty: String,
    pub start_date: NaiveDate,
    pub term: Term,
    /// 예정 종료일 (관리용). 증명서 기간 계산(`domain::certificate`)은 이 값을 보지 않는다.
    pub planned_end_date: Option<NaiveDate>,
    pub memo: String,
}

/// 저장된 경력.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Career {
    pub id: i64,
    pub uuid: String,
    pub instructor_id: i64,
    pub fields: CareerFields,
    pub import_id: Option<i64>,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Career {
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    /// `2026.03.04 ~ 현재` — 표시는 언제나 이 함수를 지난다.
    pub fn period(&self) -> String {
        date::display_period(self.fields.start_date, self.fields.term.end_date())
    }

    /// (`2026.03.04`, `현재`)
    pub fn period_parts(&self) -> (String, String) {
        date::display_period_parts(self.fields.start_date, self.fields.term.end_date())
    }
}

/// 지도사항 제안 `방과후학교 {프로그램명}` (기존 명단이 거의 이렇게 적혀 있다).
/// **제안일 뿐** 저절로 채우지 않는다 — 화면이 보여 주고 사용자가 고른다.
pub fn suggest_duty(program_name: &str) -> Option<String> {
    let p = program_name.trim();
    if p.is_empty() {
        None
    } else {
        Some(format!("방과후학교 {p}"))
    }
}

fn date_field(label: &str, s: &str) -> AppResult<NaiveDate> {
    date::parse_iso(s).map_err(|e| date_error(label, &e))
}

fn date_error(label: &str, e: &DateError) -> AppError {
    AppError::new(e.code(), format!("{label}: {}", e.message()))
}

/// 저장 전에 거치는 단 하나의 길.
pub fn prepare(raw: CareerInput) -> AppResult<CareerFields> {
    let program_name = raw.program_name.trim().to_string();
    let position = raw.position.trim().to_string();
    let duty = raw.duty.trim().to_string();
    let memo = raw.memo.trim().to_string();

    for (label, value, max) in [
        ("프로그램명", &program_name, PROGRAM_MAX),
        ("직위", &position, POSITION_MAX),
        ("지도사항", &duty, DUTY_MAX),
    ] {
        if value.is_empty() {
            return Err(AppError::invalid(format!("{label}을(를) 입력해 주세요.")));
        }
        if value.chars().count() > max {
            return Err(AppError::invalid(format!("{label}은(는) {max}자 이내로 입력해 주세요.")));
        }
    }
    if memo.chars().count() > MEMO_MAX {
        return Err(AppError::invalid(format!("메모는 {MEMO_MAX}자 이내로 입력해 주세요.")));
    }

    let start_date = date_field("시작일", &raw.start_date)?;
    let planned_end_date = match raw.planned_end_date.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => None,
        Some(s) => {
            let d = date_field("예정 종료일", s)?;
            if d < start_date {
                return Err(AppError::new(
                    "CAREER_PLANNED_BEFORE_START",
                    format!(
                        "예정 종료일({})이 시작일({})보다 빠릅니다.",
                        date::display(d),
                        date::display(start_date)
                    ),
                ));
            }
            Some(d)
        }
    };
    let end_text = raw.end_date.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let term = match raw.status {
        CareerStatus::Active => {
            if end_text.is_some() {
                return Err(AppError::new(
                    "CAREER_ACTIVE_WITH_END",
                    "재직중인 경력에는 종료일을 적지 않습니다. 종료되었다면 상태를 '종료' 로 바꿔 주세요.",
                ));
            }
            if raw.end_reason.is_some() {
                return Err(AppError::new(
                    "CAREER_ACTIVE_WITH_REASON",
                    "재직중인 경력에는 종료 사유를 적지 않습니다.",
                ));
            }
            Term::Active
        }
        CareerStatus::Ended => {
            let Some(end_text) = end_text else {
                return Err(AppError::new("CAREER_END_REQUIRED", "종료된 경력은 종료일이 필요합니다."));
            };
            let end_date = date_field("종료일", end_text)?;
            let Some(reason) = raw.end_reason else {
                return Err(AppError::new(
                    "CAREER_REASON_REQUIRED",
                    "종료 사유(계약만료 · 중도해지)를 골라 주세요.",
                ));
            };
            check_order(start_date, end_date)?;
            Term::Ended { end_date, reason }
        }
    };

    Ok(CareerFields {
        program_name,
        position,
        duty,
        start_date,
        term,
        planned_end_date,
        memo,
    })
}

fn check_order(start: NaiveDate, end: NaiveDate) -> AppResult<()> {
    if end < start {
        return Err(AppError::new(
            "CAREER_END_BEFORE_START",
            format!(
                "종료일({})이 시작일({})보다 빠릅니다.",
                date::display(end),
                date::display(start)
            ),
        ));
    }
    Ok(())
}

/// 재직중 경력을 끝낸다.
///
/// ```text
///   ACTIVE 2026-03-04 ~ (없음)  ──(종료일 2026-10-31, CONTRACT_END)──▶  ENDED 2026-03-04 ~ 2026-10-31
/// ```
///
/// * 이미 끝난 경력은 **다시 끝내지 않는다** — 종료일을 고치려는 것이라면 경력 수정으로 한다.
///   조용히 덮어쓰면 처음 종료일이 무엇이었는지 사라진다.
/// * 종료일이 시작일보다 빠르면 거부한다(같은 날은 받는다 — 하루짜리 계약).
pub fn end(current: &CareerFields, end_date: &str, reason: EndReason) -> AppResult<Term> {
    if let Term::Ended { end_date: was, .. } = current.term {
        return Err(AppError::new(
            "CAREER_ALREADY_ENDED",
            format!(
                "이미 종료된 경력입니다(종료일 {}). 종료일을 바꾸려면 경력 수정에서 고쳐 주세요.",
                date::display(was)
            ),
        ));
    }
    let end_date = date_field("종료일", end_date)?;
    check_order(current.start_date, end_date)?;
    Ok(Term::Ended { end_date, reason })
}

/// 고친 항목 이름 (변경 기록용 — 값은 담지 않는다).
pub fn changed_labels(old: &CareerFields, new: &CareerFields) -> Vec<&'static str> {
    [
        ("프로그램명", old.program_name != new.program_name),
        ("직위", old.position != new.position),
        ("지도사항", old.duty != new.duty),
        ("시작일", old.start_date != new.start_date),
        ("상태", old.term.status() != new.term.status()),
        ("종료일", old.term.end_date() != new.term.end_date()),
        ("종료 사유", old.term.reason() != new.term.reason()),
        ("예정 종료일", old.planned_end_date != new.planned_end_date),
        ("메모", old.memo != new.memo),
    ]
    .into_iter()
    .filter(|(_, changed)| *changed)
    .map(|(label, _)| label)
    .collect()
}

/// 재직중인데 예정 종료일이 오늘보다 앞인가 — 대시보드가 "종료 여부를 확인하세요" 라고 알린다.
/// **알리기만 한다.** 이 날짜로 저절로 종료하지 않는다.
pub fn planned_end_passed(f: &CareerFields, today: NaiveDate) -> bool {
    f.term == Term::Active && f.planned_end_date.is_some_and(|d| d < today)
}

/// 종료일이 오늘보다 뒤인가. 재직중이면 None.
pub fn future_end(term: &Term, today: NaiveDate) -> Option<NaiveDate> {
    term.end_date().filter(|d| *d > today)
}

/// 미래 종료일은 **막지 않되 확인을 받는다** (Phase 3 결정).
///
/// 계약 종료일이 미리 정해진 경우를 위해 허용한다. 확인 없이 들어오면 `FUTURE_END_UNCONFIRMED` —
/// 화면은 이 오류를 받으면 확인 창을 띄우고 `confirm_future = true` 로 다시 보낸다.
/// 규칙이 서버 쪽에 있으므로 화면이 확인을 빠뜨려도 저장되지 않는다.
pub fn check_future(term: &Term, today: NaiveDate, confirmed: bool) -> AppResult<()> {
    match future_end(term, today) {
        Some(d) if !confirmed => Err(AppError::new(
            "FUTURE_END_UNCONFIRMED",
            format!("종료일이 오늘 이후입니다. {}로 종료 처리하시겠습니까?", date::display(d)),
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "career_tests.rs"]
mod career_tests;
