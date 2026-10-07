use chrono::NaiveDate;

use super::*;
use crate::domain::career::{CareerFields, EndReason};

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn career(uuid: char, program: &str, start: NaiveDate, term: Term) -> Career {
    Career {
        id: 1,
        uuid: uuid.to_string().repeat(36),
        instructor_id: 1,
        fields: CareerFields {
            program_name: program.into(),
            position: "강사".into(),
            duty: "".into(),
            start_date: start,
            term,
            planned_end_date: None,
            memo: "".into(),
        },
        import_id: None,
        archived_at: None,
        created_at: "".into(),
        updated_at: "".into(),
    }
}

fn ended(d: NaiveDate) -> Term {
    Term::Ended { end_date: d, reason: EndReason::ContractEnd }
}

#[test]
fn 순서와_상관없이_같은_지문이고_uuid_순으로_놓인다() {
    let a = career('a', "미술", ymd(2026, 3, 1), ended(ymd(2026, 12, 31)));
    let b = career('b', "애니메이션", ymd(2026, 6, 1), ended(ymd(2026, 8, 31)));
    assert_eq!(pair(&a, &b), pair(&b, &a));
    assert_eq!(pair(&b, &a).a_uuid, a.uuid);
    assert_eq!(pair(&a, &b).fingerprint.len(), 64);
}

#[test]
fn 겹침_판단에_쓰는_값만_지문에_든다() {
    let a = career('a', "미술", ymd(2026, 3, 1), ended(ymd(2026, 12, 31)));
    let b = career('b', "애니메이션", ymd(2026, 6, 1), ended(ymd(2026, 8, 31)));
    let base = pair(&a, &b).fingerprint;
    let mut same = a.clone();
    same.fields.program_name = "미술 심화".into();
    same.fields.memo = "메모".into();
    same.fields.planned_end_date = Some(ymd(2027, 1, 1));
    same.fields.term = Term::Ended { end_date: ymd(2026, 12, 31), reason: EndReason::Terminated };
    assert_eq!(pair(&same, &b).fingerprint, base, "이름·메모·예정 종료일·종료 사유는 무관");
    for changed in [
        career('a', "미술", ymd(2026, 3, 2), ended(ymd(2026, 12, 31))),
        career('a', "미술", ymd(2026, 3, 1), ended(ymd(2026, 12, 30))),
        career('a', "미술", ymd(2026, 3, 1), Term::Active),
        career('c', "미술", ymd(2026, 3, 1), ended(ymd(2026, 12, 31))),
    ] {
        assert_ne!(pair(&changed, &b).fingerprint, base);
    }
}

#[test]
fn 하루라도_겹치면_겹침이고_재직중은_끝이_없다() {
    let a = career('a', "", ymd(2025, 3, 1), ended(ymd(2026, 2, 28)));
    assert!(overlaps(&a, &career('b', "", ymd(2026, 2, 28), Term::Active)));
    assert!(!overlaps(&a, &career('b', "", ymd(2026, 3, 1), Term::Active)));
    assert!(overlaps(&career('a', "", ymd(2020, 1, 1), Term::Active), &career('b', "", ymd(2030, 1, 1), Term::Active)));
}
