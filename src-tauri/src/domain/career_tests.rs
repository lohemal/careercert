use super::*;

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn active() -> CareerInput {
    CareerInput {
        program_name: "마술".into(),
        position: "강사".into(),
        duty: "방과후학교 마술".into(),
        start_date: "2026-03-04".into(),
        status: CareerStatus::Active,
        end_date: None,
        end_reason: None,
        memo: "".into(),
    }
}

fn ended(end: &str, reason: Option<EndReason>) -> CareerInput {
    CareerInput {
        start_date: "2025-03-05".into(),
        status: CareerStatus::Ended,
        end_date: Some(end.into()),
        end_reason: reason,
        ..active()
    }
}

// ---------------- 상태와 날짜 ----------------

#[test]
fn 재직중이고_종료일이_없으면_정상() {
    let f = prepare(active()).unwrap();
    assert_eq!(f.term, Term::Active);
    assert_eq!(f.start_date, ymd(2026, 3, 4));
}

#[test]
fn 재직중인데_종료일이_있으면_거부() {
    let mut v = active();
    v.end_date = Some("2026-10-31".into());
    assert_eq!(prepare(v).unwrap_err().code, "CAREER_ACTIVE_WITH_END");
}

#[test]
fn 재직중이면_빈_종료일은_없는_것으로_본다() {
    let mut v = active();
    v.end_date = Some("  ".into());
    assert_eq!(prepare(v).unwrap().term, Term::Active);
}

#[test]
fn 재직중인데_종료_사유가_있으면_거부() {
    let mut v = active();
    v.end_reason = Some(EndReason::Terminated);
    assert_eq!(prepare(v).unwrap_err().code, "CAREER_ACTIVE_WITH_REASON");
}

#[test]
fn 종료일과_사유가_있는_종료는_정상() {
    let f = prepare(ended("2026-02-06", Some(EndReason::ContractEnd))).unwrap();
    assert_eq!(
        f.term,
        Term::Ended {
            end_date: ymd(2026, 2, 6),
            reason: EndReason::ContractEnd
        }
    );
}

#[test]
fn 종료인데_종료일이_없으면_거부() {
    let mut v = ended("", Some(EndReason::ContractEnd));
    assert_eq!(prepare(v.clone()).unwrap_err().code, "CAREER_END_REQUIRED");
    v.end_date = None;
    assert_eq!(prepare(v).unwrap_err().code, "CAREER_END_REQUIRED");
}

#[test]
fn 종료인데_사유가_없으면_거부() {
    assert_eq!(
        prepare(ended("2026-02-06", None)).unwrap_err().code,
        "CAREER_REASON_REQUIRED"
    );
}

#[test]
fn 종료일이_시작일보다_빠르면_거부() {
    let e = prepare(ended("2025-03-04", Some(EndReason::ContractEnd))).unwrap_err();
    assert_eq!(e.code, "CAREER_END_BEFORE_START");
    assert!(e.user_message.contains("2025.03.04"), "{}", e.user_message);
}

#[test]
fn 시작일과_종료일이_같으면_받는다() {
    assert!(prepare(ended("2025-03-05", Some(EndReason::Terminated))).is_ok());
}

#[test]
fn 현재라는_글자는_종료일이_될_수_없다() {
    let e = prepare(ended("현재", Some(EndReason::ContractEnd))).unwrap_err();
    assert_eq!(e.code, "DATE_FORMAT");
    assert!(e.user_message.starts_with("종료일"));
}

#[test]
fn 없는_날짜는_거부() {
    let mut v = active();
    v.start_date = "2026-02-30".into();
    let e = prepare(v).unwrap_err();
    assert_eq!(e.code, "DATE_NOT_EXIST");
    assert!(e.user_message.starts_with("시작일"));
}

#[test]
fn 글자_칸은_앞뒤_공백만_정리하고_비울_수_없다() {
    let mut v = active();
    v.program_name = "  생명 과학 ".into();
    assert_eq!(prepare(v).unwrap().program_name, "생명 과학");

    for blank in ["program", "position", "duty"] {
        let mut v = active();
        match blank {
            "program" => v.program_name = " ".into(),
            "position" => v.position = "".into(),
            _ => v.duty = "\t".into(),
        }
        assert_eq!(prepare(v).unwrap_err().code, "INVALID_INPUT", "{blank}");
    }
}

// ---------------- 표시 ----------------

fn career(fields: CareerFields) -> Career {
    Career {
        id: 1,
        uuid: "x".into(),
        instructor_id: 1,
        fields,
        import_id: None,
        archived_at: None,
        created_at: "".into(),
        updated_at: "".into(),
    }
}

#[test]
fn 기간_표시는_domain_한_곳에서_만든다() {
    assert_eq!(career(prepare(active()).unwrap()).period(), "2026.03.04 ~ 현재");
    assert_eq!(
        career(prepare(ended("2026-02-06", Some(EndReason::ContractEnd))).unwrap()).period(),
        "2025.03.05 ~ 2026.02.06"
    );
}

// ---------------- 종료 처리 ----------------

#[test]
fn 재직중_경력을_종료한다() {
    let f = prepare(active()).unwrap();
    let t = end(&f, "2026-10-31", EndReason::ContractEnd).unwrap();
    assert_eq!(
        t,
        Term::Ended {
            end_date: ymd(2026, 10, 31),
            reason: EndReason::ContractEnd
        }
    );
    assert_eq!(t.status(), CareerStatus::Ended);
}

#[test]
fn 이미_종료된_경력은_다시_종료하지_않는다() {
    let f = prepare(ended("2026-02-06", Some(EndReason::ContractEnd))).unwrap();
    let e = end(&f, "2026-02-28", EndReason::Terminated).unwrap_err();
    assert_eq!(e.code, "CAREER_ALREADY_ENDED");
    assert!(e.user_message.contains("2026.02.06"), "{}", e.user_message);
}

#[test]
fn 시작일보다_이른_날로_종료할_수_없다() {
    let f = prepare(active()).unwrap();
    assert_eq!(
        end(&f, "2026-03-03", EndReason::Terminated).unwrap_err().code,
        "CAREER_END_BEFORE_START"
    );
    assert!(end(&f, "2026-03-04", EndReason::Terminated).is_ok());
}

#[test]
fn 종료일은_저장_형식으로만_받는다() {
    let f = prepare(active()).unwrap();
    assert_eq!(end(&f, "현재", EndReason::ContractEnd).unwrap_err().code, "DATE_FORMAT");
    assert_eq!(end(&f, "", EndReason::ContractEnd).unwrap_err().code, "DATE_EMPTY");
}

// ---------------- 그 밖 ----------------

#[test]
fn db_세_칸에서_상태를_읽는다() {
    assert_eq!(Term::from_columns("ACTIVE", None, None), Some(Term::Active));
    assert_eq!(
        Term::from_columns("ENDED", Some("2026-10-31"), Some("TERMINATED")),
        Some(Term::Ended {
            end_date: ymd(2026, 10, 31),
            reason: EndReason::Terminated
        })
    );
    assert_eq!(Term::from_columns("ACTIVE", Some("2026-10-31"), None), None);
    assert_eq!(Term::from_columns("ENDED", Some("현재"), Some("TERMINATED")), None);
}

#[test]
fn 지도사항은_제안만_한다() {
    assert_eq!(suggest_duty(" 마술 ").as_deref(), Some("방과후학교 마술"));
    assert_eq!(suggest_duty(""), None);
}

#[test]
fn 화면에_쓸_이름과_기본값() {
    assert_eq!(DEFAULT_POSITION, "강사");
    assert_eq!(CareerStatus::Active.label(), "재직중");
    assert_eq!(CareerStatus::Ended.label(), "종료");
    assert_eq!(EndReason::ContractEnd.label(), "계약만료");
    assert_eq!(EndReason::Terminated.label(), "중도해지");
    for r in [EndReason::ContractEnd, EndReason::Terminated] {
        assert_eq!(EndReason::from_code(r.code()), Some(r));
    }
}

#[test]
fn 미래_종료일은_확인을_받아야_저장된다() {
    let today = ymd(2026, 10, 1);
    let future = Term::Ended {
        end_date: ymd(2026, 12, 31),
        reason: EndReason::ContractEnd,
    };
    let e = check_future(&future, today, false).unwrap_err();
    assert_eq!(e.code, "FUTURE_END_UNCONFIRMED");
    assert_eq!(e.user_message, "종료일이 오늘 이후입니다. 2026.12.31로 종료 처리하시겠습니까?");
    assert!(check_future(&future, today, true).is_ok());

    let todays = Term::Ended {
        end_date: today,
        reason: EndReason::Terminated,
    };
    assert!(check_future(&todays, today, false).is_ok(), "오늘은 미래가 아니다");
    assert!(check_future(&Term::Active, today, false).is_ok());
}

#[test]
fn 바뀐_항목_이름만_돌려준다() {
    let a = prepare(active()).unwrap();
    let mut b = a.clone();
    b.duty = "다른 지도사항".into();
    b.term = Term::Ended {
        end_date: ymd(2026, 10, 31),
        reason: EndReason::ContractEnd,
    };
    assert_eq!(changed_labels(&a, &b), vec!["지도사항", "상태", "종료일", "종료 사유"]);
    assert!(changed_labels(&a, &a).is_empty());
}
