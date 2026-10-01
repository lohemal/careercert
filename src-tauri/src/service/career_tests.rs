use chrono::NaiveDate;

use super::*;
use crate::db::Db;
use crate::domain::career::{CareerStatus, Term};
use crate::domain::instructor::InstructorInput;
use crate::repo::instructor::{Filter, Scope};
use crate::service::instructor;

const NOW: &str = "2026-10-01T09:00:00";
const LATER: &str = "2026-11-01T09:00:00";

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn teacher(db: &Db, name: &str) -> i64 {
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
}

fn active(start: &str) -> CareerInput {
    CareerInput {
        program_name: "마술".into(),
        position: "강사".into(),
        duty: "방과후학교 마술".into(),
        start_date: start.into(),
        status: CareerStatus::Active,
        end_date: None,
        end_reason: None,
        memo: "".into(),
    }
}

fn ended(start: &str, end: &str) -> CareerInput {
    CareerInput {
        status: CareerStatus::Ended,
        end_date: Some(end.into()),
        end_reason: Some(EndReason::ContractEnd),
        ..active(start)
    }
}

/// 요구사항의 예: 김으뜸 경력 3건
fn kim(db: &Db) -> (i64, Vec<Career>) {
    let who = teacher(db, "김으뜸");
    // 일부러 차례를 섞어 넣는다 — 목록은 시작일 차례여야 한다
    let list = [
        active("2026-03-04"),
        ended("2024-03-08", "2025-02-07"),
        ended("2025-03-05", "2026-02-06"),
    ]
    .into_iter()
    .map(|i| db.write(|c| create(c, who, i, false, NOW)).unwrap())
    .collect();
    (who, list)
}

#[test]
fn 강사_한_명이_여러_경력을_갖고_시작일_차례로_나온다() {
    let db = Db::memory();
    let (who, _) = kim(&db);
    let list = db.read(|c| list(c, who, false)).unwrap();
    let periods: Vec<String> = list.iter().map(|c| c.period()).collect();
    assert_eq!(
        periods,
        vec![
            "2024.03.08 ~ 2025.02.07",
            "2025.03.05 ~ 2026.02.06",
            "2026.03.04 ~ 현재"
        ]
    );
}

#[test]
fn 저장된_재직중_경력의_종료일은_null이다() {
    let db = Db::memory();
    let (_, created) = kim(&db);
    let raw: (Option<String>, String, Option<String>) = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT end_date, status, end_reason FROM careers WHERE id = ?1",
                [created[0].id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .unwrap();
    assert_eq!(raw, (None, "ACTIVE".to_string(), None));
}

#[test]
fn 재직중_경력을_정상_종료한다() {
    let db = Db::memory();
    let (_, created) = kim(&db);
    let id = created[0].id; // 2026-03-04 ~ 현재
    let done = db.write(|c| end(c, id, "2026-10-31", EndReason::ContractEnd, false, LATER)).unwrap();
    assert_eq!(
        done.fields.term,
        Term::Ended {
            end_date: ymd(2026, 10, 31),
            reason: EndReason::ContractEnd
        }
    );
    assert_eq!(done.period(), "2026.03.04 ~ 2026.10.31");
    assert_eq!(done.updated_at, LATER);
    assert_eq!(done.fields.program_name, "마술", "다른 칸은 그대로");

    let raw: (String, String, String) = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT end_date, status, end_reason FROM careers WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .unwrap();
    assert_eq!(
        raw,
        ("2026-10-31".into(), "ENDED".into(), "CONTRACT_END".into())
    );
}

#[test]
fn 이미_종료된_경력의_종료는_거부하고_아무것도_바꾸지_않는다() {
    let db = Db::memory();
    let (_, created) = kim(&db);
    let id = created[1].id; // 2024-03-08 ~ 2025-02-07
    let before = db.read(|c| get(c, id)).unwrap();
    let e = db.write(|c| end(c, id, "2025-02-28", EndReason::Terminated, false, LATER)).unwrap_err();
    assert_eq!(e.code, "CAREER_ALREADY_ENDED");
    assert_eq!(db.read(|c| get(c, id)).unwrap(), before);
}

#[test]
fn 시작일보다_이른_종료는_거부한다() {
    let db = Db::memory();
    let (_, created) = kim(&db);
    let e = db
        .write(|c| end(c, created[0].id, "2026-03-03", EndReason::Terminated, false, LATER))
        .unwrap_err();
    assert_eq!(e.code, "CAREER_END_BEFORE_START");
}

#[test]
fn 수정으로_종료를_되돌릴_수_있다() {
    // 잘못 종료한 경력을 재직중으로 되돌리는 길 — 같은 prepare 검사를 지난다
    let db = Db::memory();
    let (_, created) = kim(&db);
    let id = created[0].id;
    db.write(|c| end(c, id, "2026-10-31", EndReason::Terminated, true, NOW)).unwrap();
    let back = db.write(|c| update(c, id, active("2026-03-04"), false, LATER)).unwrap();
    assert_eq!(back.fields.term, Term::Active);
    assert_eq!(back.period(), "2026.03.04 ~ 현재");
}

#[test]
fn 수정도_상태_규칙을_지킨다() {
    let db = Db::memory();
    let (_, created) = kim(&db);
    let mut bad = active("2026-03-04");
    bad.end_date = Some("2026-10-31".into());
    assert_eq!(
        db.write(|c| update(c, created[0].id, bad, false, NOW)).unwrap_err().code,
        "CAREER_ACTIVE_WITH_END"
    );
}

#[test]
fn 경력을_보관하면_기본_목록에서_빠지고_해제하면_돌아온다() {
    let db = Db::memory();
    let (who, created) = kim(&db);
    let id = created[1].id;
    db.write(|c| archive(c, id, LATER)).unwrap();

    assert_eq!(db.read(|c| list(c, who, false)).unwrap().len(), 2);
    let all = db.read(|c| list(c, who, true)).unwrap();
    assert_eq!(all.len(), 3);
    assert!(all.iter().any(|c| c.id == id && c.is_archived()));

    // 목록의 경력 수도 보관한 것을 빼고 센다
    let s = db
        .read(|c| {
            instructor::search(
                c,
                &Filter {
                    query: "김으뜸".into(),
                    scope: Scope::All,
                },
            )
        })
        .unwrap();
    assert_eq!((s[0].careers, s[0].active_careers), (2, 1));

    db.write(|c| unarchive(c, id, LATER)).unwrap();
    assert_eq!(db.read(|c| list(c, who, false)).unwrap().len(), 3);
}

#[test]
fn 보관된_경력은_고치거나_종료할_수_없다() {
    let db = Db::memory();
    let (_, created) = kim(&db);
    let id = created[0].id;
    db.write(|c| archive(c, id, NOW)).unwrap();
    assert_eq!(
        db.write(|c| end(c, id, "2026-10-31", EndReason::ContractEnd, false, NOW)).unwrap_err().code,
        "ARCHIVED"
    );
    assert_eq!(
        db.write(|c| update(c, id, active("2026-03-05"), false, NOW)).unwrap_err().code,
        "ARCHIVED"
    );
    assert_eq!(db.write(|c| archive(c, id, NOW)).unwrap_err().code, "ALREADY_ARCHIVED");
}

#[test]
fn 보관된_강사에게는_경력을_더하거나_고칠_수_없다() {
    let db = Db::memory();
    let (who, created) = kim(&db);
    db.write(|c| instructor::archive(c, who, NOW)).unwrap();
    assert_eq!(
        db.write(|c| create(c, who, active("2027-03-02"), false, NOW)).unwrap_err().code,
        "ARCHIVED"
    );
    assert_eq!(
        db.write(|c| end(c, created[0].id, "2026-10-31", EndReason::ContractEnd, false, NOW))
            .unwrap_err()
            .code,
        "ARCHIVED"
    );
    // 경력 자체는 보관되지 않았으므로 강사를 꺼내면 그대로 보인다
    db.write(|c| instructor::unarchive(c, who, NOW)).unwrap();
    assert_eq!(db.read(|c| list(c, who, false)).unwrap().len(), 3);
}

#[test]
fn 없는_강사나_경력은_찾을_수_없다() {
    let db = Db::memory();
    assert_eq!(db.write(|c| create(c, 99, active("2026-03-04"), false, NOW)).unwrap_err().code, "NOT_FOUND");
    assert_eq!(db.read(|c| list(c, 99, false)).unwrap_err().code, "NOT_FOUND");
    assert_eq!(
        db.write(|c| end(c, 99, "2026-10-31", EndReason::ContractEnd, false, NOW)).unwrap_err().code,
        "NOT_FOUND"
    );
}

#[test]
fn 거절된_등록은_아무것도_남기지_않는다() {
    let db = Db::memory();
    let who = teacher(&db, "김으뜸");
    let mut bad = ended("2026-03-04", "2026-03-01");
    bad.end_reason = Some(EndReason::Terminated);
    assert!(db.write(|c| create(c, who, bad, false, NOW)).is_err());
    assert!(db.read(|c| list(c, who, true)).unwrap().is_empty());
}

// ---------------- 미래 종료일 ----------------

#[test]
fn 미래_종료일로_종료하려면_확인이_필요하다() {
    let db = Db::memory();
    let (_, created) = kim(&db);
    let id = created[0].id; // 2026-03-04 ~ 현재, 오늘은 2026-10-01
    let e = db
        .write(|c| end(c, id, "2026-12-31", EndReason::ContractEnd, false, NOW))
        .unwrap_err();
    assert_eq!(e.code, "FUTURE_END_UNCONFIRMED");
    assert_eq!(e.user_message, "종료일이 오늘 이후입니다. 2026.12.31로 종료 처리하시겠습니까?");
    assert_eq!(db.read(|c| get(c, id)).unwrap().fields.term, Term::Active, "확인 전에는 그대로");

    let done = db
        .write(|c| end(c, id, "2026-12-31", EndReason::ContractEnd, true, NOW))
        .unwrap();
    assert_eq!(done.period(), "2026.03.04 ~ 2026.12.31");
}

#[test]
fn 수정에서도_종료일을_미래로_바꾸면_확인한다() {
    let db = Db::memory();
    let (_, created) = kim(&db);
    let id = created[0].id;
    let future = ended("2026-03-04", "2027-02-12");
    assert_eq!(
        db.write(|c| update(c, id, future.clone(), false, NOW)).unwrap_err().code,
        "FUTURE_END_UNCONFIRMED"
    );
    db.write(|c| update(c, id, future.clone(), true, NOW)).unwrap();

    // 종료일은 그대로 두고 메모만 고치면 다시 묻지 않는다
    let mut memo_only = future;
    memo_only.memo = "메모".into();
    assert!(db.write(|c| update(c, id, memo_only, false, LATER)).is_ok());
}

#[test]
fn 미래_종료일로_새_경력을_넣을_때도_확인한다() {
    let db = Db::memory();
    let who = teacher(&db, "김가람");
    let v = ended("2026-03-04", "2027-02-12");
    assert_eq!(
        db.write(|c| create(c, who, v.clone(), false, NOW)).unwrap_err().code,
        "FUTURE_END_UNCONFIRMED"
    );
    assert!(db.write(|c| create(c, who, v, true, NOW)).is_ok());
}

// ---------------- 변경 기록 ----------------

#[test]
fn 바꾼_일은_변경_기록에_남고_값은_남지_않는다() {
    use crate::repo::audit;
    let db = Db::memory();
    let who = teacher(&db, "김가람");
    let id = db.write(|c| create(c, who, active("2026-03-04"), false, NOW)).unwrap().id;
    let mut v = active("2026-03-04");
    v.duty = "비밀스러운 지도사항".into();
    v.memo = "비밀 메모".into();
    db.write(|c| update(c, id, v, false, NOW)).unwrap();
    db.write(|c| end(c, id, "2026-09-30", EndReason::Terminated, false, NOW)).unwrap();
    db.write(|c| archive(c, id, NOW)).unwrap();
    db.write(|c| unarchive(c, id, NOW)).unwrap();

    let log = db.read(|c| audit::for_target(c, "career", id)).unwrap();
    let rows: Vec<(String, String)> = log.iter().map(|e| (e.action.clone(), e.summary.clone())).collect();
    assert_eq!(
        rows,
        vec![
            ("CAREER_CREATE".into(), "등록".into()),
            ("CAREER_UPDATE".into(), "변경 항목: 지도사항, 메모".into()),
            ("CAREER_END".into(), "종료일 2026-09-30 · 중도해지".into()),
            ("CAREER_ARCHIVE".into(), "보관".into()),
            ("CAREER_UNARCHIVE".into(), "보관 해제".into()),
        ]
    );
    let all = db.read(audit::all).unwrap();
    for e in &all {
        assert!(!e.summary.contains("비밀"), "{e:?}");
        assert!(!e.summary.contains("김가람"), "{e:?}");
    }
}

#[test]
fn 바뀐_것이_없으면_기록도_수정_시각도_그대로다() {
    use crate::repo::audit;
    let db = Db::memory();
    let who = teacher(&db, "김가람");
    let id = db.write(|c| create(c, who, active("2026-03-04"), false, NOW)).unwrap().id;
    let same = db.write(|c| update(c, id, active("2026-03-04"), false, LATER)).unwrap();
    assert_eq!(same.updated_at, NOW);
    assert_eq!(db.read(|c| audit::for_target(c, "career", id)).unwrap().len(), 1);
}
