//! 이동용 백업 권장 알림 시험.

use super::*;
use crate::db::Db;
use crate::domain::career::EndReason;
use crate::domain::instructor::InstructorInput;
use crate::service::issuance::issuance_tests::{career_input, issue_now, school, world};
use crate::service::{career, instructor, issuance};

const NOW: &str = "2026-10-01T09:00:00";

fn day(s: &str) -> NaiveDate {
    date::parse_iso(s).unwrap()
}

fn state(db: &Db, today: &str) -> Reminder {
    db.read(|c| reminder(c, day(today))).unwrap()
}

#[test]
fn 자료가_없으면_알리지_않는다() {
    let db = Db::memory();
    let r = state(&db, "2026-10-01");
    assert_eq!(r.state, State::NoData);
    assert!(r.messages.is_empty());
}

#[test]
fn 자료가_있는데_한_번도_안_했으면_권장한다() {
    let w = world();
    let r = state(&w.db, "2026-10-01");
    assert_eq!(r.state, State::Never);
    assert_eq!(r.messages.len(), 1);
}

#[test]
fn 백업_뒤_바뀐_것이_없으면_조용하다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    w.db.write(|c| record_export(c, NOW)).unwrap();
    let r = state(&w.db, "2026-10-02");
    assert_eq!(r.state, State::Current);
    assert!(r.messages.is_empty());
    assert_eq!(r.last_at.as_deref(), Some(NOW));
}

#[test]
fn 강사_경력_발급_취소_설정이_바뀌면_변경으로_본다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    let mark = |w: &crate::service::issuance::issuance_tests::World| w.db.write(|c| record_export(c, NOW)).unwrap();
    let changed = |w: &crate::service::issuance::issuance_tests::World| state(&w.db, "2026-10-02").state == State::Changed;

    mark(&w);
    w.db.write(|c| instructor::create(c, InstructorInput { name: "새 강사".into(), ..Default::default() }, NOW)).unwrap();
    assert!(changed(&w), "강사 등록");

    mark(&w);
    w.db.write(|c| career::create(c, w.who, career_input("바둑", "2025-03-04", Some("2026-02-06"), None), false, NOW)).unwrap();
    assert!(changed(&w), "경력 등록");

    mark(&w);
    w.db.write(|c| career::end(c, w.active, "2026-09-30", EndReason::ContractEnd, false, NOW)).unwrap();
    assert!(changed(&w), "경력 종료");

    mark(&w);
    issue_now(&w, "제2026-153호").unwrap();
    assert!(changed(&w), "발급");

    mark(&w);
    w.db.write(|c| issuance::void(c, id, "오기", NOW)).unwrap();
    assert!(changed(&w), "취소");

    mark(&w);
    w.db.write(|c| {
        school(c, "다른 담당");
        Ok(())
    })
    .unwrap();
    // 같은 시각(NOW)으로 저장하면 시각이 같아 구분되지 않는다 — 설정은 '나중 시각' 으로 저장해 본다
    w.db.write(|c| Ok(c.execute("UPDATE school_settings SET updated_at = '2026-10-01T10:00:00'", [])?)).unwrap();
    assert!(changed(&w), "학교 설정");
}

#[test]
fn 삼십일이_지나면_보조_안내를_더한다() {
    let w = world();
    w.db.write(|c| record_export(c, NOW)).unwrap();
    assert!(!state(&w.db, "2026-10-30").stale);
    let r = state(&w.db, "2026-10-31");
    assert!(r.stale);
    assert_eq!(r.messages, vec!["마지막 이동용 백업 후 30일 이상 지났습니다.".to_string()]);
}

#[test]
fn 경로나_파일_이름은_남기지_않는다() {
    let w = world();
    w.db.write(|c| record_export(c, NOW)).unwrap();
    let values: Vec<String> = w
        .db
        .read(|c| Ok(c.prepare("SELECT value FROM app_meta WHERE key LIKE 'portable_%'")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?))
        .unwrap();
    assert_eq!(values.len(), 3);
    for v in values {
        assert!(!v.contains('\\') && !v.contains("careercert-backup"), "{v}");
    }
}
