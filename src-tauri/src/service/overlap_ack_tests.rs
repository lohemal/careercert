//! v0.1.3 경력 기간 겹침 '정상 경력으로 확인' 시험 — 한 쌍 단위, 지문(uuid·시작일·종료/재직중)으로.

use super::dashboard_tests::{add, who, NOW};
use super::*;
use crate::db::Db;
use crate::domain::career::{CareerInput, CareerStatus, EndReason};
use crate::service::career;

/// 지금 '기간이 겹치는 경력' 항목
fn overlaps(db: &Db) -> Vec<CheckItem> {
    let o = db.read(|c| overview(c, NOW)).unwrap();
    o.checks.into_iter().find(|c| c.kind == "OVERLAP").map(|c| c.items).unwrap_or_default()
}

fn ack(db: &Db, item: &CheckItem) -> crate::error::AppResult<()> {
    let r = item.overlap.as_ref().expect("겹침 항목");
    db.write(|c| career::acknowledge_overlap(c, r.career_a_id, r.career_b_id, &r.fingerprint, NOW))
}

fn input(program: &str, start: &str, end: Option<&str>, memo: &str) -> CareerInput {
    CareerInput {
        program_name: program.into(),
        position: "강사".into(),
        duty: format!("방과후학교 {program}"),
        start_date: start.into(),
        status: if end.is_some() { CareerStatus::Ended } else { CareerStatus::Active },
        end_date: end.map(String::from),
        end_reason: end.map(|_| EndReason::ContractEnd),
        planned_end_date: None,
        memo: memo.into(),
    }
}

fn update(db: &Db, id: i64, i: CareerInput) {
    db.write(|c| career::update(c, id, i, true, NOW)).unwrap();
}

/// 미술(2026.03.01~12.31) · 애니메이션(2026.06.01~08.31) — 실제로 겹치는 두 경력
fn art_and_animation(db: &Db) -> (i64, i64, i64) {
    let t = who(db, "김가람", "");
    let a = add(db, t, "미술", "2026-03-01", Some("2026-12-31"));
    let b = add(db, t, "애니메이션", "2026-06-01", Some("2026-08-31"));
    (t, a, b)
}

#[test]
fn 실제_겹침은_확인_필요에_나오고_확인하면_사라진다() {
    let db = Db::memory();
    let (_, a, b) = art_and_animation(&db);
    let items = overlaps(&db);
    assert_eq!(items.len(), 1);
    let r = items[0].overlap.as_ref().unwrap();
    assert_eq!((r.career_a_id, r.career_b_id), (a, b));
    ack(&db, &items[0]).unwrap();
    assert!(overlaps(&db).is_empty());
    let o = db.read(|c| overview(c, NOW)).unwrap();
    assert!(o.checks.iter().all(|c| c.kind != "OVERLAP"), "건수 0 이면 항목째 없다");
}

#[test]
fn 다시_열어도_숨김이_유지된다() {
    let dir = crate::db::testutil::tmp_dir("overlap-ack");
    let path = dir.join(crate::db::DB_FILE);
    {
        let db = Db::open(&path).unwrap();
        art_and_animation(&db);
        ack(&db, &overlaps(&db)[0]).unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert!(overlaps(&db).is_empty());
}

#[test]
fn 순서를_바꿔_확인해도_같은_확인이다() {
    let db = Db::memory();
    let (_, a, b) = art_and_animation(&db);
    let fp = overlaps(&db)[0].overlap.clone().unwrap().fingerprint;
    db.write(|c| career::acknowledge_overlap(c, b, a, &fp, NOW)).unwrap();
    assert!(overlaps(&db).is_empty());
    // 다시 확인해도 한 쌍에 한 행
    db.write(|c| career::acknowledge_overlap(c, a, b, &fp, NOW)).unwrap();
    let n: i64 = db.read(|c| Ok(c.query_row("SELECT COUNT(*) FROM career_overlap_acks", [], |r| r.get(0))?)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn 새_경력_c_와의_겹침은_새로_나온다() {
    let db = Db::memory();
    let (t, a, b) = art_and_animation(&db);
    ack(&db, &overlaps(&db)[0]).unwrap();
    let c = add(&db, t, "코딩", "2026-07-01", None);
    let ids: Vec<(i64, i64)> = overlaps(&db)
        .iter()
        .map(|i| i.overlap.as_ref().map(|r| (r.career_a_id, r.career_b_id)).unwrap())
        .collect();
    assert_eq!(ids.len(), 2, "a↔c, b↔c");
    assert!(ids.contains(&(a, c)) && ids.contains(&(b, c)), "{ids:?}");
    assert!(!ids.contains(&(a, b)), "확인한 a↔b 는 그대로 숨김");
}

#[test]
fn 시작일을_바꾸면_다시_나온다() {
    let db = Db::memory();
    let (_, a, _) = art_and_animation(&db);
    ack(&db, &overlaps(&db)[0]).unwrap();
    update(&db, a, input("미술", "2026-04-01", Some("2026-12-31"), ""));
    assert_eq!(overlaps(&db).len(), 1);
}

#[test]
fn 종료일을_바꾸거나_재직중으로_되돌리면_다시_나온다() {
    let db = Db::memory();
    let (_, a, _) = art_and_animation(&db);
    ack(&db, &overlaps(&db)[0]).unwrap();
    update(&db, a, input("미술", "2026-03-01", Some("2026-11-30"), ""));
    assert_eq!(overlaps(&db).len(), 1, "종료일 변경");
    ack(&db, &overlaps(&db)[0]).unwrap();
    update(&db, a, input("미술", "2026-03-01", None, ""));
    assert_eq!(overlaps(&db).len(), 1, "재직중으로");
}

#[test]
fn 더는_겹치지_않으면_나오지_않는다() {
    let db = Db::memory();
    let (_, a, _) = art_and_animation(&db);
    update(&db, a, input("미술", "2026-03-01", Some("2026-05-31"), ""));
    assert!(overlaps(&db).is_empty());
}

#[test]
fn 메모나_프로그램명만_바꾸면_확인이_유지된다() {
    let db = Db::memory();
    let (_, a, _) = art_and_animation(&db);
    ack(&db, &overlaps(&db)[0]).unwrap();
    update(&db, a, input("미술 심화", "2026-03-01", Some("2026-12-31"), "학부모 요청"));
    assert!(overlaps(&db).is_empty());
}

#[test]
fn 다른_강사의_겹침에는_영향이_없다() {
    let db = Db::memory();
    art_and_animation(&db);
    let other = who(&db, "이나래", "");
    add(&db, other, "미술", "2026-03-01", Some("2026-12-31"));
    add(&db, other, "애니메이션", "2026-06-01", Some("2026-08-31"));
    let items = overlaps(&db);
    assert_eq!(items.len(), 2);
    ack(&db, items.iter().find(|i| i.instructor_name == "김가람").unwrap()).unwrap();
    let left = overlaps(&db);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].instructor_name, "이나래");
}

#[test]
fn 바뀐_지문이나_겹치지_않는_쌍이나_다른_강사끼리는_확인할_수_없다() {
    let db = Db::memory();
    let (t, a, b) = art_and_animation(&db);
    let stale = overlaps(&db)[0].overlap.clone().unwrap().fingerprint;
    update(&db, b, input("애니메이션", "2026-06-02", Some("2026-08-31"), ""));
    let code = |r: crate::error::AppResult<()>| r.unwrap_err().code;
    assert_eq!(code(db.write(|c| career::acknowledge_overlap(c, a, b, &stale, NOW))), "OVERLAP_CHANGED");
    let far = add(&db, t, "로봇", "2020-03-01", Some("2020-12-31"));
    assert_eq!(code(db.write(|c| career::acknowledge_overlap(c, a, far, &stale, NOW))), "OVERLAP_CHANGED");
    let other = who(&db, "이나래", "");
    let o = add(&db, other, "미술", "2026-03-01", None);
    assert_eq!(code(db.write(|c| career::acknowledge_overlap(c, a, o, &stale, NOW))), "INVALID_INPUT");
    assert_eq!(overlaps(&db).len(), 1);
}

#[test]
fn 감사_기록에는_내부_번호만_남고_저장한_값에_개인정보가_없다() {
    let db = Db::memory();
    let (_, a, b) = art_and_animation(&db);
    ack(&db, &overlaps(&db)[0]).unwrap();
    let log = db.read(crate::repo::audit::all).unwrap();
    let e = log.iter().find(|e| e.action == "CAREER_OVERLAP_ACK").unwrap();
    assert_eq!(e.summary, format!("경력 기간 겹침 확인 완료 · 경력 #{} ↔ #{}", a.min(b), a.max(b)));
    assert_eq!(e.target_type, "career");
    let cols: Vec<String> = db
        .read(|c| {
            Ok(c.prepare("SELECT name FROM pragma_table_info('career_overlap_acks')")?
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?)
        })
        .unwrap();
    assert_eq!(cols, vec!["id", "career_a_uuid", "career_b_uuid", "fingerprint", "acknowledged_at"]);
    let dump: String = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT group_concat(career_a_uuid || career_b_uuid || fingerprint || acknowledged_at) FROM career_overlap_acks",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    for bad in ["김가람", "미술", "애니메이션", "2026-03-01"] {
        assert!(!dump.contains(bad), "{bad}");
    }
}

#[test]
fn 다른_확인_필요_항목은_숨길_수_없고_그대로다() {
    let db = Db::memory();
    let t = who(&db, "김가람", "");
    who(&db, "김가람", "");
    add(&db, t, "미술", "2026-03-01", Some("2026-12-31"));
    add(&db, t, "애니메이션", "2026-06-01", Some("2026-08-31"));
    add(&db, t, "코딩", "2027-03-01", None);
    let o = db.read(|c| overview(c, NOW)).unwrap();
    let before: Vec<&str> = o.checks.iter().map(|c| c.kind).collect();
    assert!(before.contains(&"DUPLICATE_NAME") && before.contains(&"ACTIVE_STARTS_LATER") && before.contains(&"OVERLAP"));
    for c in o.checks.iter().filter(|c| c.kind != "OVERLAP") {
        assert!(c.items.iter().all(|i| i.overlap.is_none()), "{} 에는 확인 버튼이 없다", c.kind);
    }
    for i in &o.checks.iter().find(|c| c.kind == "OVERLAP").unwrap().items {
        ack(&db, i).unwrap();
    }
    let after: Vec<&str> = db.read(|c| overview(c, NOW)).unwrap().checks.iter().map(|c| c.kind).collect();
    let expect: Vec<&str> = before.into_iter().filter(|k| *k != "OVERLAP").collect();
    assert_eq!(after, expect);
}
