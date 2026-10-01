//! 날짜 — 저장 형식, 표시 형식, 기존 엑셀 글자 해석.
//!
//! **기간 표시(`2026.03.04 ~ 현재`)를 만드는 곳은 여기 하나뿐이다.** 화면·증명서·이력이
//! 모두 이 함수를 지난다. 화면이 따로 만들면 한쪽만 고쳐져 증명서와 화면이 어긋난다.
//!
//! 세 가지 입구
//!   * `parse_iso`        — 앱 안에서 오가는 값(`YYYY-MM-DD`). 엄격하다.
//!   * `parse_loose`      — 기존 엑셀에 사람이 적은 글자(`2022.3.4.` 등). 너그럽지만 없는 날짜는 거부.
//!   * `is_current_word`  — 종료 칸의 `현재`·`재직중`. **날짜가 아니다** — 종료일 NULL 로 바꾼다.

use chrono::{Datelike, NaiveDate};

/// 재직중 경력의 종료 쪽에 찍는 말. 저장하지 않고 **표시할 때만** 쓴다.
pub const CURRENT_LABEL: &str = "현재";

/// 받아들이는 연도 범위. 이 밖은 오타(예: `0222`, `2202`)로 본다.
pub const MIN_YEAR: i32 = 1950;
pub const MAX_YEAR: i32 = 2099;

/// 날짜를 읽지 못한 까닭.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateError {
    /// 빈칸
    Empty,
    /// 날짜 모양이 아니다 (`3월초`, `2022.3`, `현재` …)
    Format,
    /// 모양은 맞지만 없는 날짜 (`2022.02.30`, `2022.13.04`)
    NotExist,
    /// 연도가 범위 밖
    YearOutOfRange,
}

impl DateError {
    pub fn code(&self) -> &'static str {
        match self {
            DateError::Empty => "DATE_EMPTY",
            DateError::Format => "DATE_FORMAT",
            DateError::NotExist => "DATE_NOT_EXIST",
            DateError::YearOutOfRange => "DATE_YEAR_RANGE",
        }
    }

    pub fn message(&self) -> String {
        match self {
            DateError::Empty => "날짜가 비어 있습니다.".into(),
            DateError::Format => "날짜 형식이 아닙니다. 예: 2026.03.04".into(),
            DateError::NotExist => "존재하지 않는 날짜입니다.".into(),
            DateError::YearOutOfRange => {
                format!("연도는 {MIN_YEAR}~{MAX_YEAR} 사이여야 합니다.")
            }
        }
    }
}

// ---------------------------------------------------------------
// 저장 형식
// ---------------------------------------------------------------

/// DB 에 넣는 모양 `2026-03-04`.
pub fn to_iso(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// 앱 안에서 오가는 `YYYY-MM-DD` 만 받는다(0 채움 필수).
pub fn parse_iso(s: &str) -> Result<NaiveDate, DateError> {
    let t = s.trim();
    if t.is_empty() {
        return Err(DateError::Empty);
    }
    let b = t.as_bytes();
    let shape = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if !shape {
        return Err(DateError::Format);
    }
    let y: i32 = t[0..4].parse().map_err(|_| DateError::Format)?;
    let m: u32 = t[5..7].parse().map_err(|_| DateError::Format)?;
    let d: u32 = t[8..10].parse().map_err(|_| DateError::Format)?;
    make(y, m, d)
}

// ---------------------------------------------------------------
// 표시 형식
// ---------------------------------------------------------------

/// 화면·증명서에 찍는 모양 `2026.03.04`.
pub fn display(d: NaiveDate) -> String {
    format!("{:04}.{:02}.{:02}", d.year(), d.month(), d.day())
}

/// 공문서 날짜 표기 `2026. 10. 1.` — 증명서의 발급일 줄에 쓴다(행정 문서의 날짜 쓰는 법).
pub fn display_official(d: NaiveDate) -> String {
    format!("{}. {}. {}.", d.year(), d.month(), d.day())
}

/// 종료 쪽 표시. 종료일이 없으면 `현재`.
pub fn display_end(end: Option<NaiveDate>) -> String {
    match end {
        Some(d) => display(d),
        None => CURRENT_LABEL.to_string(),
    }
}

/// 기간 한 줄 `2026.03.04 ~ 현재` / `2025.03.05 ~ 2026.02.06`.
pub fn display_period(start: NaiveDate, end: Option<NaiveDate>) -> String {
    let (from, to) = display_period_parts(start, end);
    format!("{from} ~ {to}")
}

/// 기간의 두 쪽 (`2026.03.04`, `현재`). 증명서의 '부터 · 까지' 칸, 좁은 화면의 줄바꿈에 쓴다.
/// `display_period` 도 이것을 지나므로 한 줄 표시와 두 쪽 표시가 어긋날 수 없다.
pub fn display_period_parts(start: NaiveDate, end: Option<NaiveDate>) -> (String, String) {
    (display(start), display_end(end))
}

// ---------------------------------------------------------------
// 기존 엑셀 글자
// ---------------------------------------------------------------

/// 종료 칸에 적힌 "아직 근무 중" 이라는 말인가. 공백은 무시한다.
///
/// 이 말들은 **날짜로 저장하지 않는다** — 가져오기가 종료일 NULL + 재직중으로 바꾼다.
#[cfg_attr(not(test), allow(dead_code))] // Phase 8 엑셀 가져오기가 쓴다
pub fn is_current_word(s: &str) -> bool {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    matches!(t.as_str(), "현재" | "재직중" | "재직" | "근무중")
}

/// 사람이 적은 날짜 글자를 읽는다.
///
/// 받는 모양 (앞뒤 공백 무시)
///   * `2022.3.4.` `2022.03.04` `2022. 3. 4` — 점, 끝 점·사이 공백 허용
///   * `2022-03-04` `2022-3-4`, `2022/3/4`
///   * `2022년 3월 4일`
///   * `20220304` — 숫자 8자리
///   * `22.3.4` — 두 자리 연도는 **2000년대**로 읽는다(설계안 §9). 강사 계약 기간이므로
///     1900년대일 수 없다.
///
/// 받지 않는 것
///   * 구분자를 섞은 것 (`2022.3-4`) — 실수일 가능성이 커서 추측하지 않는다
///   * 세 토막이 아닌 것 (`2022.3`), 월·일이 세 자리 이상, 숫자가 아닌 글자
///   * 없는 날짜 (`2022.02.30`, `2022.13.04`)
#[cfg_attr(not(test), allow(dead_code))] // Phase 8 엑셀 가져오기가 쓴다
pub fn parse_loose(s: &str) -> Result<NaiveDate, DateError> {
    let t = s.trim();
    if t.is_empty() {
        return Err(DateError::Empty);
    }

    // 숫자 8자리
    if t.len() == 8 && t.bytes().all(|c| c.is_ascii_digit()) {
        let y: i32 = t[0..4].parse().map_err(|_| DateError::Format)?;
        let m: u32 = t[4..6].parse().map_err(|_| DateError::Format)?;
        let d: u32 = t[6..8].parse().map_err(|_| DateError::Format)?;
        return make(y, m, d);
    }

    // `2022년 3월 4일` → `2022.3.4`
    let korean = t.contains('년') || t.contains('월') || t.contains('일');
    let unified: String = if korean {
        let mut u = t.replace('년', ".").replace('월', ".");
        if let Some(stripped) = u.trim_end().strip_suffix('일') {
            u = stripped.to_string();
        }
        if u.contains('일') {
            return Err(DateError::Format);
        }
        u
    } else {
        t.to_string()
    };

    // 구분자는 한 가지만
    let seps: Vec<char> = ['.', '-', '/']
        .into_iter()
        .filter(|c| unified.contains(*c))
        .collect();
    if seps.len() != 1 {
        return Err(DateError::Format);
    }
    let sep = seps[0];

    let mut parts: Vec<&str> = unified.split(sep).map(str::trim).collect();
    // 끝의 구분자 하나는 허용 (`2022.3.4.`)
    if parts.len() == 4 && parts[3].is_empty() {
        parts.pop();
    }
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit())) {
        return Err(DateError::Format);
    }
    let (ys, ms, ds) = (parts[0], parts[1], parts[2]);
    if !(ys.len() == 4 || ys.len() == 2) || ms.len() > 2 || ds.len() > 2 {
        return Err(DateError::Format);
    }

    let mut y: i32 = ys.parse().map_err(|_| DateError::Format)?;
    if ys.len() == 2 {
        y += 2000;
    }
    let m: u32 = ms.parse().map_err(|_| DateError::Format)?;
    let d: u32 = ds.parse().map_err(|_| DateError::Format)?;
    make(y, m, d)
}

/// 시각 `YYYY-MM-DDTHH:MM:SS` 의 날짜. service 는 시각(`now`) 하나만 받고 오늘은 여기서 뽑는다 —
/// 시험에서 시각을 고정하면 "오늘" 도 함께 고정된다.
pub fn today_of(now: &str) -> Result<NaiveDate, DateError> {
    parse_iso(now.get(0..10).unwrap_or(""))
}

fn make(y: i32, m: u32, d: u32) -> Result<NaiveDate, DateError> {
    if !(MIN_YEAR..=MAX_YEAR).contains(&y) {
        return Err(DateError::YearOutOfRange);
    }
    NaiveDate::from_ymd_opt(y, m, d).ok_or(DateError::NotExist)
}

#[cfg(test)]
#[path = "date_tests.rs"]
mod date_tests;
