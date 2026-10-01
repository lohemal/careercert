use super::*;
use crate::db::Db;
use crate::domain::career::{CareerInput, CareerStatus, EndReason};
use crate::domain::instructor::InstructorInput;
use crate::service::{career, instructor};

const NOW: &str = "2026-10-01T09:00:00";

fn who(db: &Db, name: &str, distinguisher: &str) -> i64 {
    db.write(|c| {
        instructor::create(
            c,
            InstructorInput {
                name: name.into(),
                distinguisher: distinguisher.into(),
                ..Default::default()
            },
            NOW,
        )
    })
    .unwrap()
    .id
}

fn add(db: &Db, owner: i64, program: &str, start: &str, end: Option<&str>) -> i64 {
    db.write(|c| {
        career::create(
            c,
            owner,
            CareerInput {
                program_name: program.into(),
                position: "강사".into(),
                duty: format!("방과후학교 {program}"),
                start_date: start.into(),
                status: if end.is_some() { CareerStatus::Ended } else { CareerStatus::Active },
                end_date: end.map(String::from),
                end_reason: end.map(|_| EndReason::ContractEnd),
                planned_end_date: None,
                memo: "".into(),
            },
            true,
            NOW,
        )
    })
    .unwrap()
    .id
}

fn kinds(o: &Overview) -> Vec<&'static str> {
    o.checks.iter().map(|c| c.kind).collect()
}

#[test]
fn 깨끗한_자료에는_확인_항목이_없다() {
    let db = Db::memory();
    let a = who(&db, "김가람", "");
    add(&db, a, "마술", "2025-03-05", Some("2026-02-06"));
    add(&db, a, "마술", "2026-03-04", None);
    let o = db.read(|c| overview(c, NOW)).unwrap();
    assert_eq!((o.instructors, o.active_careers), (1, 1));
    assert!(o.checks.is_empty(), "{:?}", kinds(&o));
}

#[test]
fn 동명이인인데_구분_메모가_없으면_알린다() {
    let db = Db::memory();
    let a = who(&db, "김가람", "");
    who(&db, "김가람", "1985년생");
    who(&db, "이나래", "");
    let o = db.read(|c| overview(c, NOW)).unwrap();
    assert_eq!(kinds(&o), vec!["DUPLICATE_NAME"]);
    let items = &o.checks[0].items;
    assert_eq!(items.len(), 1, "메모가 있는 쪽과 혼자인 이름은 걸리지 않는다");
    assert_eq!(items[0].instructor_id, a);
}

#[test]
fn 날짜로_가릴_수_있는_것만_알린다() {
    let db = Db::memory();
    let a = who(&db, "김가람", "");
    add(&db, a, "마술", "2026-11-01", None); // 시작이 오늘 뒤
    let b = who(&db, "이나래", "");
    add(&db, b, "바둑", "2026-03-04", Some("2026-12-31")); // 종료 예정
    let c = who(&db, "박다솜", "");
    add(&db, c, "미술", "2026-03-04", None);
    add(&db, c, "과학", "2026-09-01", None); // 겹침
    let d = who(&db, "최라온", "");
    add(&db, d, "체육", "2020-03-02", None); // 오래 재직중이지만 판단하지 않는다

    let o = db.read(|cn| overview(cn, NOW)).unwrap();
    assert_eq!(kinds(&o), vec!["ACTIVE_STARTS_LATER", "ENDS_LATER", "OVERLAP"]);
    assert_eq!(o.checks[0].items[0].instructor_id, a);
    assert_eq!(o.checks[0].items[0].detail, "마술 · 2026.11.01 ~ 현재");
    assert_eq!(o.checks[1].items[0].detail, "바둑 · 2026.03.04 ~ 2026.12.31");
    assert_eq!(o.checks[2].items[0].instructor_id, c);
    assert!(o.checks.iter().flat_map(|k| &k.items).all(|i| i.instructor_id != d));
}

#[test]
fn 이어지는_계약은_겹침이_아니다() {
    let db = Db::memory();
    let a = who(&db, "김가람", "");
    add(&db, a, "마술", "2025-03-05", Some("2026-02-06"));
    add(&db, a, "마술", "2026-02-07", None);
    assert!(db.read(|c| overview(c, NOW)).unwrap().checks.is_empty());
}

#[test]
fn 보관한_것은_보지_않는다() {
    let db = Db::memory();
    let a = who(&db, "김가람", "");
    let b = who(&db, "김가람", "");
    let late = add(&db, b, "마술", "2026-11-01", None);
    db.write(|c| instructor::archive(c, a, NOW)).unwrap();
    db.write(|c| career::archive(c, late, NOW)).unwrap();
    let o = db.read(|c| overview(c, NOW)).unwrap();
    assert!(o.checks.is_empty(), "{:?}", kinds(&o));
    assert_eq!((o.instructors, o.active_careers), (1, 0));
}

fn add_planned(db: &Db, owner: i64, start: &str, planned: &str) -> i64 {
    db.write(|c| {
        career::create(
            c,
            owner,
            CareerInput {
                program_name: "마술".into(),
                position: "강사".into(),
                duty: "방과후학교 마술".into(),
                start_date: start.into(),
                status: CareerStatus::Active,
                end_date: None,
                end_reason: None,
                planned_end_date: Some(planned.into()),
                memo: "".into(),
            },
            false,
            NOW,
        )
    })
    .unwrap()
    .id
}

#[test]
fn 예정_종료일이_지난_재직중_경력을_알리고_아무것도_바꾸지_않는다() {
    let db = Db::memory();
    let a = who(&db, "김가람", "");
    let late = add_planned(&db, a, "2025-03-05", "2026-02-06"); // 오늘(2026-10-01) 보다 앞
    let b = who(&db, "이나래", "");
    add_planned(&db, b, "2026-03-04", "2027-02-05"); // 아직

    let o = db.read(|c| overview(c, NOW)).unwrap();
    assert_eq!(kinds(&o), vec!["PLANNED_END_PASSED"]);
    assert_eq!(o.checks[0].description, "예정 종료일이 지났습니다. 종료 여부를 확인하세요. 프로그램이 저절로 종료하지 않습니다.");
    assert_eq!(o.checks[0].items.len(), 1);
    assert_eq!(o.checks[0].items[0].detail, "마술 · 2025.03.05 ~ 현재 · 예정 종료일 2026.02.06");
    // 알리기만 한다
    assert_eq!(db.read(|c| career::get(c, late)).unwrap().fields.term, crate::domain::career::Term::Active);
    assert_eq!(o.active_careers, 2);
}

#[test]
fn 이미_종료한_경력은_예정_종료일이_지나도_알리지_않는다() {
    let db = Db::memory();
    let a = who(&db, "김가람", "");
    let id = add_planned(&db, a, "2025-03-05", "2026-02-06");
    db.write(|c| career::end(c, id, "2026-02-06", EndReason::ContractEnd, false, NOW)).unwrap();
    assert!(db.read(|c| overview(c, NOW)).unwrap().checks.is_empty());
}
