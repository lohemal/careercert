//! 가상값만 쓴다. 주민번호 모양 줄에는 `privacy:fake`.

use super::*;
use crate::db::testutil::tmp_dir;
use crate::db::{Db, DB_FILE};
use crate::domain::career::{CareerInput, CareerStatus, EndReason};
use crate::domain::instructor::InstructorInput;
use crate::domain::settings::{SettingsInput, DEFAULT_PURPOSE, DEFAULT_TITLE};
use crate::service::{career, instructor};

const NOW: &str = "2026-10-01T09:00:00";
const FAKE_RRN: &str = "880808-2000002"; // privacy:fake
const FAKE_ADDRESS: &str = "가상시 연습구 시험로 123-45";

fn setup(db: &Db) -> (i64, Vec<i64>) {
    db.write(|c| {
        settings_repo::save(
            c,
            SettingsInput {
                school_name: "○○초등학교".into(),
                certificate_title: DEFAULT_TITLE.into(),
                issuer_title: "".into(),
                department: "방과후학교".into(),
                manager_name: "가상담당".into(),
                phone: "000-0000-0000".into(),
                default_purpose: DEFAULT_PURPOSE.into(),
            },
            NOW,
        )
    })
    .unwrap();
    let who = db
        .write(|c| {
            instructor::create(
                c,
                InstructorInput {
                    name: "김가람".into(),
                    ..Default::default()
                },
                NOW,
            )
        })
        .unwrap()
        .id;
    let add = |start: &str, end: Option<&str>, planned: Option<&str>| {
        db.write(|c| {
            career::create(
                c,
                who,
                CareerInput {
                    program_name: "마술".into(),
                    position: "강사".into(),
                    duty: "방과후학교 마술".into(),
                    start_date: start.into(),
                    status: if end.is_some() { CareerStatus::Ended } else { CareerStatus::Active },
                    end_date: end.map(String::from),
                    end_reason: end.map(|_| EndReason::ContractEnd),
                    planned_end_date: planned.map(String::from),
                    memo: "".into(),
                },
                true,
                NOW,
            )
        })
        .unwrap()
        .id
    };
    let ids = vec![
        add("2025-03-05", Some("2026-02-06"), None),
        add("2026-03-04", None, Some("2027-02-05")),
        add("2026-11-02", None, None), // 오늘(2026-10-01) 보다 뒤에 시작
    ];
    (who, ids)
}

fn issue(on: &str) -> IssueInput {
    IssueInput {
        rrn: FAKE_RRN.into(),
        address: FAKE_ADDRESS.into(),
        issue_no: " 제2026-152호 ".into(),
        purpose: "기관제출".into(),
        issued_on: on.into(),
    }
}

#[test]
fn 고를_수_있는_경력은_보관하지_않은_것이고_발급일에_따라_바뀐다() {
    let db = Db::memory();
    let (who, ids) = setup(&db);
    db.write(|c| career::archive(c, ids[0], NOW)).unwrap();

    let list = db.read(|c| choices(c, who, "2026-10-01")).unwrap();
    let got: Vec<(i64, bool)> = list.iter().map(|x| (x.career.id, x.on_issue.is_ok())).collect();
    assert_eq!(got, vec![(ids[1], true), (ids[2], false)], "보관한 경력은 목록에 없다");
    assert_eq!(list[1].on_issue, Err(Ineligible::NotStarted));

    // 발급일을 뒤로 바꾸면 다시 고를 수 있다
    let later = db.read(|c| choices(c, who, "2026-11-02")).unwrap();
    assert!(later.iter().all(|x| x.on_issue.is_ok()));
}

#[test]
fn 지금_자료와_학교_설정으로_내용을_만든다() {
    let db = Db::memory();
    let (who, ids) = setup(&db);
    let b = db
        .read(|c| {
            prepare(
                c,
                PrepareRequest {
                    instructor_id: who,
                    career_ids: vec![ids[1], ids[0]],
                    issue: issue("2026-10-01"),
                },
                NOW,
            )
        })
        .unwrap();
    assert_eq!(b.doc.issue_no, "제2026-152호");
    assert_eq!(b.doc.school.issuer_title, "○○초등학교장");
    let rows: Vec<(&str, &str)> = b.doc.items.iter().map(|i| (i.from_text.as_str(), i.to_text.as_str())).collect();
    assert_eq!(rows, vec![("2025.03.05", "2026.02.06"), ("2026.03.04", "현재")]);
}

#[test]
fn 발급일_뒤에_시작하는_경력은_넣을_수_없다() {
    let db = Db::memory();
    let (who, ids) = setup(&db);
    let e = db
        .read(|c| {
            prepare(
                c,
                PrepareRequest {
                    instructor_id: who,
                    career_ids: vec![ids[1], ids[2]],
                    issue: issue("2026-10-01"),
                },
                NOW,
            )
        })
        .unwrap_err();
    assert_eq!(e.code, "CERT_CAREER_NOT_STARTED");
}

#[test]
fn 작성_중인_주민번호와_주소는_db에_남지_않는다() {
    // 실제 파일 DB 로 — WAL 파일까지 본다
    let dir = tmp_dir("cert-no-pii");
    let path = dir.join(DB_FILE);
    let db = Db::open(&path).unwrap();
    let (who, ids) = setup(&db);
    for _ in 0..3 {
        let _ = db.read(|c| {
            prepare(
                c,
                PrepareRequest {
                    instructor_id: who,
                    career_ids: ids[..2].to_vec(),
                    issue: issue("2026-10-01"),
                },
                NOW,
            )
        });
    }
    // 실패하는 요청도 흔적을 남기지 않는다
    let _ = db.read(|c| {
        prepare(
            c,
            PrepareRequest {
                instructor_id: who,
                career_ids: vec![],
                issue: issue("2026-10-01"),
            },
            NOW,
        )
    });

    let log_rows: i64 = db
        .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM audit_log WHERE action LIKE 'CERT%'", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(log_rows, 0, "증명서 작성은 변경 기록을 남기지 않는다");
    drop(db);

    let digits: String = FAKE_RRN.chars().filter(|c| c.is_ascii_digit()).collect();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_file() {
            let bytes = std::fs::read(&p).unwrap();
            let hay = String::from_utf8_lossy(&bytes);
            assert!(!hay.contains(FAKE_RRN), "{} 에 주민번호", p.display());
            assert!(!hay.contains(&digits), "{} 에 주민번호(숫자만)", p.display());
            assert!(!hay.contains(FAKE_ADDRESS), "{} 에 주소", p.display());
        }
    }
}

#[test]
fn 요청의_디버그_출력은_주민번호와_주소를_가린다() {
    let r = PrepareRequest {
        instructor_id: 1,
        career_ids: vec![1],
        issue: issue("2026-10-01"),
    };
    let s = format!("{r:?}");
    assert!(!s.contains("880808")); // privacy:fake
    assert!(!s.contains("가상시"));
    assert!(s.contains("제2026-152호"));
}
