//! 같은 강사의 경력 기간 겹침 — 판정과 '정상 경력으로 확인' 의 지문.
//!
//! 겹침 판정은 시작일과 끝(종료 경력은 종료일, 재직중은 끝 없음)만 본다. 예정 종료일(planned_end_date)·
//! 프로그램명·종료 사유·메모는 보지 않는다. 그래서 지문도 **판정에 쓰는 값만** 담는다:
//!
//! ```text
//!   careercert/overlap/v1
//!   {uuid 작은 쪽}|{시작일}|{ACTIVE | ENDED:종료일}
//!   {uuid 큰 쪽}|{시작일}|{ACTIVE | ENDED:종료일}
//! ```
//! → SHA-256 16진수. 두 경력을 uuid 순으로 놓으므로 A/B 순서와 상관없이 같은 지문이다.
//! 시작일·종료일·재직중↔종료가 바뀌면 지문이 달라져 다시 '확인 필요' 에 나온다. 메모·프로그램명만 바꾸면 그대로.

use chrono::NaiveDate;

use super::career::{Career, Term};

/// 두 기간이 하루라도 겹치는가. 재직중은 끝이 없는 것으로 본다.
pub fn overlaps(a: &Career, b: &Career) -> bool {
    let end = |c: &Career| c.fields.term.end_date().unwrap_or(NaiveDate::MAX);
    a.fields.start_date <= end(b) && b.fields.start_date <= end(a)
}

/// 확인 지문의 대상 한 쌍 (uuid 순)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub a_uuid: String,
    pub b_uuid: String,
    pub fingerprint: String,
}

fn part(c: &Career) -> String {
    let term = match c.fields.term {
        Term::Active => "ACTIVE".to_string(),
        Term::Ended { end_date, .. } => format!("ENDED:{end_date}"),
    };
    format!("{}|{}|{term}", c.uuid, c.fields.start_date)
}

pub fn pair(x: &Career, y: &Career) -> Pair {
    let (a, b) = if x.uuid <= y.uuid { (x, y) } else { (y, x) };
    let text = format!("careercert/overlap/v1\n{}\n{}", part(a), part(b));
    Pair {
        a_uuid: a.uuid.clone(),
        b_uuid: b.uuid.clone(),
        fingerprint: crate::crypto::plain_sha256(text.as_bytes()),
    }
}

#[cfg(test)]
#[path = "overlap_tests.rs"]
mod overlap_tests;
