use super::*;
use crate::db::Db;
use crate::domain::career::{CareerInput, CareerStatus};
use crate::domain::instructor::InstructorInput;
use crate::service::{career as career_service, instructor};

const NOW: &str = "2026-10-01T09:00:00";

/// 강사 셋, 재직중 경력 넷, 종료 경력 하나
struct World {
    db: Db,
    a1: i64, // 김가람 마술 2026-03-04 ~ 현재
    a2: i64, // 김가람 과학 2026-09-01 ~ 현재
    b1: i64, // 이나래 바둑 2026-03-04 ~ 현재
    c1: i64, // 박다솜 미술 2025-03-05 ~ 현재
    ended: i64,
    c_owner: i64,
}

fn world() -> World {
    let db = Db::memory();
    let who = |name: &str| {
        db.write(|c| {
            instructor::create(
                c,
                InstructorInput {
                    name: name.into(),
                    ..Default::default()
                },
                NOW,
            )
        })
        .unwrap()
        .id
    };
    let (kim, lee, park) = (who("김가람"), who("이나래"), who("박다솜"));
    let add = |owner: i64, program: &str, start: &str, end: Option<&str>| {
        db.write(|c| {
            career_service::create(
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
                    memo: "".into(),
                },
                false,
                NOW,
            )
        })
        .unwrap()
        .id
    };
    let a1 = add(kim, "마술", "2026-03-04", None);
    let a2 = add(kim, "과학", "2026-09-01", None);
    let b1 = add(lee, "바둑", "2026-03-04", None);
    let c1 = add(park, "미술", "2025-03-05", None);
    let ended = add(park, "미술", "2024-03-05", Some("2025-02-07"));
    World {
        db,
        a1,
        a2,
        b1,
        c1,
        ended,
        c_owner: park,
    }
}

fn req(ids: &[i64], end: &str) -> BulkEndRequest {
    BulkEndRequest {
        career_ids: ids.to_vec(),
        end_date: end.into(),
        reason: EndReason::ContractEnd,
    }
}

fn status_of(w: &World, id: i64) -> CareerStatus {
    w.db.read(|c| repo::get(c, id)).unwrap().fields.term.status()
}

#[test]
fn 후보는_재직중이고_보관되지_않은_경력뿐이다() {
    let w = world();
    let list = w.db.read(candidates).unwrap();
    let ids: Vec<i64> = list.iter().map(|c| c.career.id).collect();
    // 강사 이름 차례(김 → 박 → 이), 같은 강사는 시작일 차례
    assert_eq!(ids, vec![w.a1, w.a2, w.c1, w.b1]);
    assert!(!ids.contains(&w.ended));

    w.db.write(|c| career_service::archive(c, w.a2, NOW)).unwrap();
    w.db.write(|c| instructor::archive(c, w.c_owner, NOW)).unwrap();
    let ids: Vec<i64> = w.db.read(candidates).unwrap().iter().map(|c| c.career.id).collect();
    assert_eq!(ids, vec![w.a1, w.b1]);
}

#[test]
fn 아무것도_고르지_않으면_미리보기도_없다() {
    let w = world();
    assert_eq!(
        w.db.read(|c| preview(c, &req(&[], "2027-02-12"), NOW)).unwrap_err().code,
        "BULK_EMPTY"
    );
}

#[test]
fn 미리보기는_변경_전후를_보여_주고_아무것도_쓰지_않는다() {
    let w = world();
    let p = w.db.read(|c| preview(c, &req(&[w.b1, w.a1], "2027-02-12"), NOW)).unwrap();
    assert!(p.ok);
    assert!(p.future, "2027-02-12 는 2026-10-01 보다 뒤");
    let rows: Vec<(String, String, Option<String>)> = p
        .items
        .iter()
        .map(|i| (i.instructor_name.clone(), i.before.clone(), i.after.clone()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("김가람".into(), "2026.03.04 ~ 현재".into(), Some("2026.03.04 ~ 2027.02.12".into())),
            ("이나래".into(), "2026.03.04 ~ 현재".into(), Some("2026.03.04 ~ 2027.02.12".into())),
        ]
    );
    assert_eq!(status_of(&w, w.a1), CareerStatus::Active);
}

#[test]
fn 고른_것만_한_번에_종료한다() {
    let w = world();
    let r = req(&[w.a1, w.b1], "2026-09-30");
    let p = w.db.read(|c| preview(c, &r, NOW)).unwrap();
    assert!(!p.future);
    let done = w.db.write(|c| apply(c, &r, &p.state_key, false, NOW)).unwrap();
    assert_eq!(done.ended, 2);
    assert_eq!(status_of(&w, w.a1), CareerStatus::Ended);
    assert_eq!(status_of(&w, w.b1), CareerStatus::Ended);
    assert_eq!(status_of(&w, w.a2), CareerStatus::Active, "고르지 않은 것은 그대로");
    assert_eq!(status_of(&w, w.c1), CareerStatus::Active);
    assert_eq!(w.db.read(|c| count_ended_on(c, "2026-09-30")).unwrap(), 2);
}

#[test]
fn 미리보기_뒤에_자료가_바뀌면_적용하지_않는다() {
    let w = world();
    let r = req(&[w.a1, w.b1], "2026-09-30");
    let p = w.db.read(|c| preview(c, &r, NOW)).unwrap();

    // 그사이 누군가 한 건을 고쳤다
    let mut v = CareerInput {
        program_name: "바둑".into(),
        position: "강사".into(),
        duty: "방과후학교 바둑 (고침)".into(),
        start_date: "2026-03-04".into(),
        status: CareerStatus::Active,
        end_date: None,
        end_reason: None,
        memo: "".into(),
    };
    v.memo = "고침".into();
    w.db.write(|c| career_service::update(c, w.b1, v, false, "2026-10-01T09:30:00"))
        .unwrap();

    let e = w.db.write(|c| apply(c, &r, &p.state_key, false, NOW)).unwrap_err();
    assert_eq!(e.code, "BULK_STALE");
    assert_eq!(status_of(&w, w.a1), CareerStatus::Active, "한 건도 바뀌지 않았다");
}

#[test]
fn 한_건이라도_끝낼_수_없으면_전부_취소한다() {
    let w = world();
    // c1 은 2025-03-05 시작 — 종료일 2025-12-31 은 괜찮지만 a1(2026-03-04 시작)은 안 된다
    let r = req(&[w.a1, w.c1], "2025-12-31");
    let p = w.db.read(|c| preview(c, &r, NOW)).unwrap();
    assert!(!p.ok);
    assert_eq!(p.problems(), 1);
    let bad = p.items.iter().find(|i| i.career_id == w.a1).unwrap();
    assert!(bad.problem.as_deref().unwrap().contains("시작일"), "{:?}", bad.problem);
    assert_eq!(bad.after, None);

    let e = w.db.write(|c| apply(c, &r, &p.state_key, false, NOW)).unwrap_err();
    assert_eq!(e.code, "BULK_INVALID");
    assert_eq!(status_of(&w, w.c1), CareerStatus::Active, "끝낼 수 있던 것도 그대로");
}

#[test]
fn 이미_종료된_경력이나_보관된_경력이_섞이면_막는다() {
    let w = world();
    w.db.write(|c| career_service::archive(c, w.a2, NOW)).unwrap();
    let p = w.db.read(|c| preview(c, &req(&[w.a1, w.a2, w.ended, 999], "2026-09-30"), NOW)).unwrap();
    let problems: Vec<(i64, bool)> = p.items.iter().map(|i| (i.career_id, i.problem.is_some())).collect();
    assert_eq!(problems, vec![(w.a1, false), (w.a2, true), (w.ended, true), (999, true)]);
    assert!(!p.ok);
}

#[test]
fn 미래_종료일은_확인해야_적용된다() {
    let w = world();
    let r = req(&[w.a1], "2027-02-12");
    let p = w.db.read(|c| preview(c, &r, NOW)).unwrap();
    let e = w.db.write(|c| apply(c, &r, &p.state_key, false, NOW)).unwrap_err();
    assert_eq!(e.code, "FUTURE_END_UNCONFIRMED");
    assert!(e.user_message.contains("2027.02.12"), "{}", e.user_message);
    assert_eq!(status_of(&w, w.a1), CareerStatus::Active);

    assert!(w.db.write(|c| apply(c, &r, &p.state_key, true, NOW)).is_ok());
    assert_eq!(status_of(&w, w.a1), CareerStatus::Ended);
}

#[test]
fn 일괄_종료는_변경_기록에_남고_이름은_남지_않는다() {
    let w = world();
    let r = req(&[w.a1, w.b1], "2026-09-30");
    let p = w.db.read(|c| preview(c, &r, NOW)).unwrap();
    w.db.write(|c| apply(c, &r, &p.state_key, false, NOW)).unwrap();

    let log = w.db.read(audit::all).unwrap();
    let bulk: Vec<_> = log.iter().filter(|e| e.action == "BULK_END").collect();
    assert_eq!(bulk.len(), 1);
    assert_eq!(bulk[0].summary, "재직중 경력 2건 일괄 종료 · 종료일 2026-09-30 · 계약만료");
    let per: Vec<_> = log
        .iter()
        .filter(|e| e.action == "CAREER_END")
        .map(|e| e.target_id)
        .collect();
    assert_eq!(per, vec![Some(w.a1), Some(w.b1)]);
    for e in &log {
        for name in ["김가람", "이나래", "박다솜", "마술", "바둑"] {
            assert!(!e.summary.contains(name), "{e:?}");
        }
    }
}

#[test]
fn 같은_번호를_두_번_골라도_한_번만_종료한다() {
    let w = world();
    let r = req(&[w.a1, w.a1], "2026-09-30");
    let p = w.db.read(|c| preview(c, &r, NOW)).unwrap();
    assert_eq!(p.items.len(), 1);
    assert_eq!(w.db.write(|c| apply(c, &r, &p.state_key, false, NOW)).unwrap().ended, 1);
}
