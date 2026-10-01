//! 주민등록번호 — **형식만** 본다 (Phase 4 결정).
//!
//! 지키는 것
//!   * 오류 문구에 입력값을 넣지 않는다. `Debug` 도 값을 찍지 않는다(`Rrn(******)`).
//!     로그·변경 기록·오류 detail 어디로도 새지 않게 하려는 것이다.
//!   * 끝자리 검증 공식은 쓰지 않는다 — 2020년 10월 이후 부여된 번호에는 맞지 않는다.
//!   * 숫자 13자리가 아니면 거부한다. 생년월일이 이상한 것은 **경고만** 한다(외국인 등록번호·
//!     오래된 번호에 예외가 있을 수 있어 사람이 판단한다).
//!   * 하이픈·공백은 무시하고 `YYMMDD-NNNNNNN` 모양으로 맞춘다. 숫자는 바꾸지 않는다.

use chrono::NaiveDate;

use crate::error::AppError;

/// 형식을 통과한 주민등록번호. `YYMMDD-NNNNNNN`.
#[derive(Clone, PartialEq, Eq)]
pub struct Rrn(String);

impl std::fmt::Debug for Rrn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Rrn(******)")
    }
}

impl Rrn {
    /// 증명서에 찍는 모양 `YYMMDD-NNNNNNN`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 목록용 가린 모양 `YYMMDD-N******` (Phase 6 발급이력이 쓴다).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn masked(&self) -> String {
        format!("{}******", &self.0[..8])
    }

    /// 앞 6자리와 성별 자리로 본 생년월일이 실제 있는 날인가.
    pub fn birth_date_ok(&self) -> bool {
        let d = self.0.as_bytes();
        let num = |a: usize, b: usize| -> u32 { self.0[a..b].parse().unwrap_or(0) };
        let century = match d[7] {
            b'1' | b'2' | b'5' | b'6' => 1900,
            b'3' | b'4' | b'7' | b'8' => 2000,
            b'9' | b'0' => 1800,
            _ => return false,
        };
        NaiveDate::from_ymd_opt(century + num(0, 2) as i32, num(2, 4), num(4, 6)).is_some()
    }
}

/// 입력을 읽는다. 실패 문구에는 입력값을 넣지 않는다.
pub fn parse(s: &str) -> Result<Rrn, AppError> {
    let digits: String = s.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if digits.is_empty() {
        return Err(AppError::new("RRN_REQUIRED", "주민등록번호를 입력해 주세요."));
    }
    let hyphens = s.chars().filter(|c| *c == '-').count();
    if digits.chars().count() != 13 || !digits.bytes().all(|c| c.is_ascii_digit()) || hyphens > 1 {
        return Err(AppError::new(
            "RRN_FORMAT",
            "주민등록번호는 숫자 13자리입니다 (예: 앞 6자리-뒤 7자리).",
        ));
    }
    Ok(Rrn(format!("{}-{}", &digits[..6], &digits[6..])))
}

#[cfg(test)]
#[path = "rrn_tests.rs"]
mod rrn_tests;
