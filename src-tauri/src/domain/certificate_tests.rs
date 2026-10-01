//! 가상값만 쓴다. 주민번호 모양 줄에는 `privacy:fake`.

use super::*;
use crate::domain::career::{CareerFields, EndReason};
use crate::domain::settings::{DEFAULT_PURPOSE, DEFAULT_TITLE};

const FAKE_RRN: &str = "900101-1000001"; // privacy:fake

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn who() -> Instructor {
    Instructor {
        id: 7,
        uuid: "u".repeat(36),
        name: "김가람".into(),
        distinguisher: "1985년생".into(),
        phone: "".into(),
        memo: "".into(),
        archived_at: None,
        created_at: "".into(),
        updated_at: "".into(),
    }
}

fn school() -> SchoolSettings {
    SchoolSettings {
        school_name: "○○초등학교".into(),
        certificate_title: DEFAULT_TITLE.into(),
        issuer_title: "○○초등학교장".into(),
        department: "방과후학교".into(),
        manager_name: "가상담당".into(),
        phone: "000-0000-0000".into(),
        default_purpose: DEFAULT_PURPOSE.into(),
        updated_at: None,
    }
}

fn career(id: i64, start: NaiveDate, term: Term) -> Career {
    Career {
        id,
        uuid: "c".repeat(36),
        instructor_id: 7,
        fields: CareerFields {
            program_name: "마술".into(),
            position: "강사".into(),
            duty: "방과후학교 마술".into(),
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

fn ended(end: NaiveDate) -> Term {
    Term::Ended {
        end_date: end,
        reason: EndReason::ContractEnd,
    }
}

fn input(issued_on: &str) -> IssueInput {
    IssueInput {
        rrn: FAKE_RRN.into(),
        address: "○○시 ○○로 1".into(),
        issue_no: "제2026-152호".into(),
        purpose: "기관제출".into(),
        issued_on: issued_on.into(),
    }
}

fn today() -> NaiveDate {
    ymd(2026, 10, 1)
}

// ---------------- 발급일 기준 기간 ----------------

#[test]
fn 재직중_경력은_발급일_기준으로_현재() {
    let c = career(1, ymd(2026, 3, 4), Term::Active);
    assert_eq!(end_on(&c.fields, ymd(2026, 10, 1)), Ok(None));
    assert_eq!(end_on(&c.fields, ymd(2026, 3, 4)), Ok(None), "시작한 날 발급도 된다");
}

#[test]
fn 종료_경력은_발급일이_종료일_이후면_종료일을_쓴다() {
    let c = career(1, ymd(2026, 3, 4), ended(ymd(2026, 12, 31)));
    assert_eq!(end_on(&c.fields, ymd(2027, 1, 10)), Ok(Some(ymd(2026, 12, 31))));
    assert_eq!(end_on(&c.fields, ymd(2026, 12, 31)), Ok(Some(ymd(2026, 12, 31))), "종료일 당일은 종료일");
}

#[test]
fn 종료_경력도_발급일이_종료일보다_앞이면_현재() {
    // 요구사항의 예: 2026-03-04 ~ 2026-12-31 을 2026-10-01 에 발급하면 '현재'
    let c = career(1, ymd(2026, 3, 4), ended(ymd(2026, 12, 31)));
    assert_eq!(end_on(&c.fields, ymd(2026, 10, 1)), Ok(None));
}

#[test]
fn 발급일에_아직_시작하지_않은_경력은_넣을_수_없다() {
    let c = career(1, ymd(2026, 9, 1), Term::Active);
    assert_eq!(end_on(&c.fields, ymd(2026, 8, 1)), Err(Ineligible::NotStarted));
    assert_eq!(end_on(&c.fields, ymd(2026, 9, 1)), Ok(None), "발급일을 바꾸면 다시 넣을 수 있다");
}

#[test]
fn 예정_종료일은_기간_계산에_쓰지_않는다() {
    let mut a = career(1, ymd(2026, 3, 4), Term::Active);
    a.fields.planned_end_date = Some(ymd(2026, 9, 1)); // 발급일보다 앞이어도
    assert_eq!(end_on(&a.fields, ymd(2026, 10, 1)), Ok(None), "재직중이면 '현재'");

    let mut b = career(2, ymd(2026, 3, 4), ended(ymd(2026, 12, 31)));
    b.fields.planned_end_date = Some(ymd(2027, 2, 5));
    assert_eq!(end_on(&b.fields, ymd(2027, 1, 10)), Ok(Some(ymd(2026, 12, 31))), "실제 종료일을 쓴다");
}

#[test]
fn 보관한_경력은_넣을_수_없다() {
    let mut c = career(1, ymd(2026, 3, 4), Term::Active);
    c.archived_at = Some("2026-10-01T09:00:00".into());
    assert_eq!(eligibility(&c, ymd(2026, 10, 1)), Err(Ineligible::Archived));
}

// ---------------- build ----------------

#[test]
fn 증명서_내용을_발급일_기준으로_만든다() {
    let list = [
        career(3, ymd(2026, 3, 4), ended(ymd(2026, 12, 31))),
        career(1, ymd(2024, 3, 8), ended(ymd(2025, 2, 7))),
        career(2, ymd(2025, 3, 5), ended(ymd(2026, 2, 6))),
    ];
    let b = build(&who(), &list, input("2026-10-01"), &school(), today()).unwrap();
    let d = &b.doc;
    assert_eq!(d.template_version, TEMPLATE_VERSION);
    assert_eq!(d.title, DEFAULT_TITLE);
    assert_eq!(d.issue_no, "제2026-152호");
    assert_eq!(d.issued_on, ymd(2026, 10, 1));
    assert_eq!(d.purpose, "기관제출");
    assert_eq!(d.holder.name, "김가람", "구분 메모는 증명서에 들어가지 않는다");
    assert_eq!(d.holder.rrn.as_str(), FAKE_RRN);
    assert_eq!(d.holder.address, "○○시 ○○로 1");
    let rows: Vec<(i64, &str, &str)> = d
        .items
        .iter()
        .map(|i| (i.source_career_id, i.from_text.as_str(), i.to_text.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (1, "2024.03.08", "2025.02.07"),
            (2, "2025.03.05", "2026.02.06"),
            (3, "2026.03.04", "현재"),
        ],
        "시작일 차례, 발급일 기준 끝"
    );
    assert_eq!(
        d.school,
        SchoolBlock {
            issuer_title: "○○초등학교장".into(),
            department: "방과후학교".into(),
            manager_name: "가상담당".into(),
            phone: "000-0000-0000".into(),
        }
    );
    // 실제 종료일이 발급일 뒤인 경력은 알린다
    assert_eq!(
        b.warnings.iter().map(|w| (w.code, w.career_id)).collect::<Vec<_>>(),
        vec![("ENDS_AFTER_ISSUE", Some(3))]
    );
}

#[test]
fn 같은_경력을_발급일만_바꿔_만들면_끝이_달라진다() {
    let list = [career(1, ymd(2026, 3, 4), ended(ymd(2026, 12, 31)))];
    let early = build(&who(), &list, input("2026-10-01"), &school(), today()).unwrap();
    let late = build(&who(), &list, input("2027-01-10"), &school(), ymd(2027, 1, 10)).unwrap();
    assert_eq!(early.doc.items[0].to_text, "현재");
    assert_eq!(late.doc.items[0].to_text, "2026.12.31");
}

#[test]
fn 고른_경력이_없으면_만들지_않는다() {
    assert_eq!(
        build(&who(), &[], input("2026-10-01"), &school(), today()).unwrap_err().code,
        "CERT_NO_CAREERS"
    );
}

#[test]
fn 보관한_경력이나_아직_시작하지_않은_경력이_섞이면_만들지_않는다() {
    let mut archived = career(2, ymd(2025, 3, 5), Term::Active);
    archived.archived_at = Some("x".into());
    let ok = career(1, ymd(2024, 3, 8), Term::Active);
    assert_eq!(
        build(&who(), &[ok.clone(), archived], input("2026-10-01"), &school(), today()).unwrap_err().code,
        "CERT_CAREER_ARCHIVED"
    );
    let later = career(3, ymd(2026, 9, 1), Term::Active);
    let e = build(&who(), &[ok, later], input("2026-08-01"), &school(), today()).unwrap_err();
    assert_eq!(e.code, "CERT_CAREER_NOT_STARTED");
    assert!(e.user_message.contains("2026.08.01"), "{}", e.user_message);
}

#[test]
fn 다른_강사의_경력은_넣을_수_없다() {
    let mut other = career(1, ymd(2024, 3, 8), Term::Active);
    other.instructor_id = 99;
    assert_eq!(
        build(&who(), &[other], input("2026-10-01"), &school(), today()).unwrap_err().code,
        "CERT_CAREER_OTHER"
    );
}

#[test]
fn 보관된_강사의_증명서는_만들지_않는다() {
    let mut w = who();
    w.archived_at = Some("x".into());
    let list = [career(1, ymd(2024, 3, 8), Term::Active)];
    assert_eq!(
        build(&w, &list, input("2026-10-01"), &school(), today()).unwrap_err().code,
        "CERT_INSTRUCTOR_ARCHIVED"
    );
}

#[test]
fn 발급번호는_비울_수_없고_앞뒤_공백_말고는_그대로_쓴다() {
    let list = [career(1, ymd(2024, 3, 8), Term::Active)];
    let mut v = input("2026-10-01");
    v.issue_no = "   ".into();
    assert_eq!(build(&who(), &list, v, &school(), today()).unwrap_err().code, "CERT_ISSUE_NO_REQUIRED");

    for (typed, kept) in [
        ("  제2026-152호  ", "제2026-152호"),
        ("2026-152", "2026-152"),
        ("제 2026 - 152 호", "제 2026 - 152 호"),
        ("NEIS-0001/가", "NEIS-0001/가"),
    ] {
        let mut v = input("2026-10-01");
        v.issue_no = typed.into();
        assert_eq!(build(&who(), &list, v, &school(), today()).unwrap().doc.issue_no, kept, "{typed:?}");
    }
}

#[test]
fn 주민번호와_주소와_용도와_발급일을_검사한다() {
    let list = [career(1, ymd(2024, 3, 8), Term::Active)];
    let cases: [(fn(&mut IssueInput), &str); 5] = [
        (|v| v.rrn = "".into(), "RRN_REQUIRED"),
        (|v| v.rrn = "1234".into(), "RRN_FORMAT"),
        (|v| v.address = "  ".into(), "CERT_ADDRESS_REQUIRED"),
        (|v| v.purpose = "".into(), "CERT_PURPOSE_REQUIRED"),
        (|v| v.issued_on = "2026-02-30".into(), "DATE_NOT_EXIST"),
    ];
    for (change, code) in cases {
        let mut v = input("2026-10-01");
        change(&mut v);
        let e = build(&who(), &list, v, &school(), today()).unwrap_err();
        assert_eq!(e.code, code);
        assert!(!e.user_message.contains("900101"), "오류에 주민번호가 새면 안 된다"); // privacy:fake
        assert!(!e.user_message.contains("○○로"), "오류에 주소가 새면 안 된다");
    }
}

#[test]
fn 학교_설정이_비어_있으면_만들지_않는다() {
    let list = [career(1, ymd(2024, 3, 8), Term::Active)];
    let mut s = school();
    s.phone = "".into();
    s.issuer_title = "".into();
    let e = build(&who(), &list, input("2026-10-01"), &s, today()).unwrap_err();
    assert_eq!(e.code, "CERT_SCHOOL_INCOMPLETE");
    assert!(e.user_message.contains("발급자 표기, 전화번호"), "{}", e.user_message);
}

#[test]
fn 학교_설정의_지금_값이_들어간다() {
    let list = [career(1, ymd(2024, 3, 8), Term::Active)];
    let mut s = school();
    s.certificate_title = "방과후학교 강사 경력증명서".into();
    s.manager_name = "새담당".into();
    let d = build(&who(), &list, input("2026-10-01"), &s, today()).unwrap().doc;
    assert_eq!(d.title, "방과후학교 강사 경력증명서");
    assert_eq!(d.school.manager_name, "새담당");
}

#[test]
fn 발급일이_미래거나_너무_과거면_알린다() {
    let list = [career(1, ymd(2024, 3, 8), Term::Active)];
    let codes = |on: &str| -> Vec<&'static str> {
        build(&who(), &list, input(on), &school(), today())
            .unwrap()
            .warnings
            .iter()
            .map(|w| w.code)
            .collect()
    };
    assert_eq!(codes("2026-10-02"), vec!["ISSUE_IN_FUTURE"]);
    assert_eq!(codes("2025-09-30"), vec!["ISSUE_FAR_PAST"]);
    assert!(codes("2026-10-01").is_empty());
}

#[test]
fn 생년월일이_이상한_주민번호는_막지_않고_알린다() {
    let list = [career(1, ymd(2024, 3, 8), Term::Active)];
    let mut v = input("2026-10-01");
    v.rrn = "901301-1000001".into(); // privacy:fake (13월)
    let b = build(&who(), &list, v, &school(), today()).unwrap();
    assert_eq!(b.warnings.iter().map(|w| w.code).collect::<Vec<_>>(), vec!["RRN_BIRTH_DATE"]);
    assert!(!b.warnings[0].message.contains("9013"));
}

#[test]
fn 디버그_출력에_주민번호와_주소가_나오지_않는다() {
    let list = [career(1, ymd(2024, 3, 8), Term::Active)];
    let v = input("2026-10-01");
    assert!(!format!("{v:?}").contains("900101")); // privacy:fake
    let b = build(&who(), &list, v, &school(), today()).unwrap();
    let dump = format!("{b:?}");
    assert!(!dump.contains("900101"), "{dump}"); // privacy:fake
    assert!(!dump.contains("○○로"), "{dump}");
}
