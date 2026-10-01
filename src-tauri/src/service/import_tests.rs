//! 엑셀 가져오기 시험. 시험용 엑셀은 코드로 만든다(가상값만).

use rust_xlsxwriter::{Format, Workbook};

use super::*;
use crate::db::testutil::tmp_dir;
use crate::db::{backup, Db, DB_FILE};
use crate::domain::instructor::InstructorInput;
use crate::service::{career as career_service, instructor as instructor_service};

const NOW: &str = "2026-10-01T09:00:00";
const FAKE_RRN: &str = "900101-1000001"; // privacy:fake

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
}

/// 칸 값
enum V {
    S(&'static str),
    /// 날짜 형식을 입힌 엑셀 날짜 숫자
    D(f64),
    /// 날짜 형식 없는 숫자
    N(f64),
    E,
}

const HEAD: [&str; 6] = ["성명", "프로그램명", "직위", "시작날짜", "마감날짜", "지도사항"];

/// 시트 `강사명단`: 1행 제목, 2행 머리글, 3행부터 자료 (+ 주민번호·주소 열이 섞여 있다)
fn xlsx(rows: &[[V; 6]]) -> Vec<u8> {
    let mut wb = Workbook::new();
    let date = Format::new().set_num_format("yyyy-mm-dd");
    {
        let ws = wb.add_worksheet();
        ws.set_name("안내").unwrap();
        ws.write_string(0, 0, "이 시트는 설명입니다").unwrap();
    }
    let ws = wb.add_worksheet();
    ws.set_name("강사명단").unwrap();
    ws.write_string(0, 0, "방과후 강사 명단 (가상)").unwrap();
    for (j, h) in HEAD.iter().enumerate() {
        ws.write_string(1, j as u16, *h).unwrap();
    }
    ws.write_string(1, 6, "주민번호").unwrap();
    ws.write_string(1, 7, "주소").unwrap();
    for (i, r) in rows.iter().enumerate() {
        let row = i as u32 + 2;
        for (j, v) in r.iter().enumerate() {
            let col = j as u16;
            match v {
                V::S(s) => {
                    ws.write_string(row, col, *s).unwrap();
                }
                V::D(n) => {
                    ws.write_number_with_format(row, col, *n, &date).unwrap();
                }
                V::N(n) => {
                    ws.write_number(row, col, *n).unwrap();
                }
                V::E => {}
            }
        }
        ws.write_string(row, 6, FAKE_RRN).unwrap();
        ws.write_string(row, 7, "가상시 연습구 시험로 9").unwrap();
    }
    wb.save_to_buffer().unwrap()
}

fn row(name: &'static str, program: &'static str, start: V, end: V) -> [V; 6] {
    [V::S(name), V::S(program), V::S("강사"), start, end, V::S("방과후학교 지도")]
}

fn read_x(bytes: &[u8]) -> Extract {
    read_bytes(bytes, SourceKind::Xlsx, None).unwrap()
}

fn status(a: &Analysis, row_no: u32) -> RowStatus {
    a.rows.iter().find(|p| p.row_no == row_no).unwrap().status
}

fn new_all(a: &Analysis) -> Decisions {
    Decisions {
        groups: a
            .groups
            .iter()
            .map(|g| GroupDecision { name: g.name.clone(), action: Some(GroupAction::New), instructor_id: None, distinguisher: "가져온 강사".into() })
            .collect(),
        ..Default::default()
    }
}

fn count(db: &Db, sql: &str) -> i64 {
    db.read(|c| Ok(c.query_row(sql, [], |r| r.get(0))?)).unwrap()
}

// ---------------- 읽기 ----------------

#[test]
fn 강사명단_시트와_머리글_줄을_찾고_대응한_열만_꺼낸다() {
    let bytes = xlsx(&[row("김가상", "마술", V::S("2022.3.4."), V::S("2023.2.10"))]);
    let ex = read_x(&bytes);
    assert_eq!(ex.sheet, "강사명단");
    assert_eq!(ex.sheets, vec!["안내", "강사명단"]);
    assert_eq!(ex.header_row, 2);
    assert_eq!(ex.rows.len(), 1);
    assert_eq!(ex.rows[0].row_no, 3);
    assert_eq!(ex.columns.iter().map(|c| c.2.as_str()).collect::<Vec<_>>(), vec!["A", "B", "C", "D", "E", "F"]);
    let dump = format!("{ex:?}");
    assert!(!dump.contains(FAKE_RRN) && !dump.contains("시험로"), "주민번호·주소 열은 꺼내지 않는다");
}

#[test]
fn xlsm_도_같은_방식으로_읽는다() {
    let bytes = xlsx(&[row("김가상", "마술", V::S("2022.3.4."), V::S("2023.2.10"))]);
    let path = tmp_dir("import-xlsm").join("명단.xlsm");
    std::fs::write(&path, &bytes).unwrap();
    let ex = read(&path, None).unwrap();
    assert_eq!(ex.kind, SourceKind::Xlsm);
    assert_eq!(ex.rows.len(), 1);
    let csv = tmp_dir("import-csv").join("명단.csv");
    std::fs::write(&csv, "성명").unwrap();
    assert!(read(&csv, None).is_err(), "엑셀 통합 문서만");
}

#[test]
fn 필요한_열이_없으면_알린다() {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "성명").unwrap();
    ws.write_string(0, 1, "시작날짜").unwrap();
    let e = read_bytes(&wb.save_to_buffer().unwrap(), SourceKind::Xlsx, None).unwrap_err();
    assert!(e.user_message.contains("프로그램명"), "{}", e.user_message);
}

// ---------------- 날짜 ----------------

#[test]
fn 날짜_글자와_엑셀_날짜_숫자를_읽는다() {
    let db = Db::memory();
    let bytes = xlsx(&[
        row("가", "마술", V::S("2022.3.4."), V::S("2023. 2. 10")),
        row("나", "마술", V::D(44624.0), V::D(44967.0)),     // 날짜 형식 칸 2022-03-04 ~ 2023-02-10
        row("다", "마술", V::N(44624.0), V::N(20230210.0)),  // 날짜 형식 없는 숫자, 8자리 숫자
        row("라", "마술", V::S("22.3.4"), V::S("2023-02-10")),
    ]);
    let a = db.read(|c| analyze(c, &read_x(&bytes), today())).unwrap();
    for p in &a.rows {
        assert_eq!(p.status, RowStatus::Ok, "{}행 {:?}", p.row_no, p.messages);
        assert_eq!(p.start, NaiveDate::from_ymd_opt(2022, 3, 4));
        assert_eq!(p.end, End::Date(NaiveDate::from_ymd_opt(2023, 2, 10).unwrap()));
    }
    assert_eq!(a.formats.get("엑셀 날짜"), Some(&2));
    assert_eq!(a.formats.get("엑셀 날짜 숫자"), Some(&1));
    assert_eq!(a.formats.get("숫자 8자리"), Some(&1));
}

#[test]
fn 현재_계열은_재직중이고_빈_종료일은_검토_필요다() {
    let db = Db::memory();
    let bytes = xlsx(&[
        row("가", "마술", V::S("2026.3.4"), V::S("현재")),
        row("나", "마술", V::S("2026.3.4"), V::S("재직 중")),
        row("다", "마술", V::S("2026.3.4"), V::S("근무중")),
        row("라", "마술", V::S("2026.3.4"), V::E),
    ]);
    let a = db.read(|c| analyze(c, &read_x(&bytes), today())).unwrap();
    for no in [3, 4, 5] {
        assert_eq!(status(&a, no), RowStatus::Ok);
        assert_eq!(a.rows.iter().find(|p| p.row_no == no).unwrap().end, End::Current);
    }
    assert_eq!(status(&a, 6), RowStatus::Review, "빈 종료일을 재직중이라 단정하지 않는다");
}

#[test]
fn 잘못된_날짜와_빈_칸은_오류다() {
    let db = Db::memory();
    let bytes = xlsx(&[
        row("가", "마술", V::S("2022.13.4"), V::S("2023.2.10")),
        row("나", "마술", V::S("2022.2.30"), V::S("2023.2.10")),
        row("다", "마술", V::S("2023.3.4"), V::S("2022.2.10")),
        row("라", "마술", V::S("3월초"), V::S("현재")),
        [V::S("마"), V::E, V::S("강사"), V::S("2022.3.4"), V::S("현재"), V::S("지도")],
    ]);
    let a = db.read(|c| analyze(c, &read_x(&bytes), today())).unwrap();
    for no in 3..=7 {
        assert_eq!(status(&a, no), RowStatus::Error, "{no}행");
    }
}

#[test]
fn 파일_안의_같은_경력은_한_번만() {
    let db = Db::memory();
    let bytes = xlsx(&[row("가", "마술", V::S("2022.3.4"), V::S("2023.2.10")), row("가", "마술", V::S("2022.03.04"), V::S("2023.2.10"))]);
    let a = db.read(|c| analyze(c, &read_x(&bytes), today())).unwrap();
    assert_eq!(status(&a, 4), RowStatus::Duplicate);
}

// ---------------- 매칭·결정 ----------------

fn existing(db: &Db, name: &str, dist: &str) -> i64 {
    db.write(|c| instructor_service::create(c, InstructorInput { name: name.into(), distinguisher: dist.into(), phone: "000-0000-0000".into(), memo: "".into() }, NOW))
        .unwrap()
        .id
}

#[test]
fn 같은_이름이_있어도_자동으로_연결하지_않는다() {
    let db = Db::memory();
    let id = existing(&db, "김가상", "");
    let bytes = xlsx(&[row("김가상", "마술", V::S("2022.3.4"), V::S("2023.2.10"))]);
    let a = db.read(|c| analyze(c, &read_x(&bytes), today())).unwrap();
    assert_eq!(a.groups[0].candidates.iter().map(|c| c.id).collect::<Vec<_>>(), vec![id]);
    let p = db.read(|c| plan(c, &a, &Decisions::default())).unwrap();
    assert!(!p.problems.is_empty(), "결정 없이는 반영할 수 없다");
    assert!(p.ops.is_empty());
}

#[test]
fn 기존_강사와_연결하면_그_강사에게_넣고_이미_있는_경력은_건너뛴다() {
    let db = Db::memory();
    let id = existing(&db, "김가상", "");
    db.write(|c| {
        career_service::create(
            c,
            id,
            career::CareerInput {
                program_name: "마술".into(),
                position: "강사".into(),
                duty: "방과후학교 지도".into(),
                start_date: "2022-03-04".into(),
                status: CareerStatus::Ended,
                end_date: Some("2023-02-10".into()),
                end_reason: Some(EndReason::ContractEnd),
                planned_end_date: None,
                memo: "".into(),
            },
            false,
            NOW,
        )
    })
    .unwrap();
    let bytes = xlsx(&[row("김가상", "마술", V::S("2022.3.4"), V::S("2023.2.10")), row("김가상", "마술", V::S("2023.3.4"), V::S("2024.2.10"))]);
    let ex = read_x(&bytes);
    let a = db.read(|c| analyze(c, &ex, today())).unwrap();
    let d = Decisions {
        groups: vec![GroupDecision { name: "김가상".into(), action: Some(GroupAction::Link), instructor_id: Some(id), distinguisher: "".into() }],
        ..Default::default()
    };
    let p = db.read(|c| plan(c, &a, &d)).unwrap();
    assert!(p.problems.is_empty(), "{:?}", p.problems);
    assert_eq!(p.outcomes[&3], Outcome::SkipDuplicateDb);
    assert_eq!(p.outcomes[&4], Outcome::Add);
    assert_eq!((p.summary.instructors_linked, p.summary.instructors_created, p.summary.careers_added), (1, 0, 1));
    let r = db.write(|c| apply(c, &ex, &d, &p.fingerprint, NOW)).unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM instructors"), 1, "새 강사를 만들지 않았다");
    assert_eq!(count(&db, &format!("SELECT count(*) FROM careers WHERE instructor_id = {id}")), 2);
    assert_eq!(count(&db, &format!("SELECT count(*) FROM careers WHERE import_id = {}", r.import_id)), 1);
}

#[test]
fn 같은_이름이_있는데_새_강사로_등록하려면_구분_메모가_필요하다() {
    let db = Db::memory();
    existing(&db, "김가상", "");
    let bytes = xlsx(&[row("김가상", "마술", V::S("2022.3.4"), V::S("2023.2.10"))]);
    let ex = read_x(&bytes);
    let a = db.read(|c| analyze(c, &ex, today())).unwrap();
    let mut d = Decisions {
        groups: vec![GroupDecision { name: "김가상".into(), action: Some(GroupAction::New), instructor_id: None, distinguisher: "".into() }],
        ..Default::default()
    };
    assert!(!db.read(|c| plan(c, &a, &d)).unwrap().problems.is_empty());
    d.groups[0].distinguisher = "1985년생".into();
    let p = db.read(|c| plan(c, &a, &d)).unwrap();
    assert!(p.problems.is_empty());
    let r = db.write(|c| apply(c, &ex, &d, &p.fingerprint, NOW)).unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM instructors WHERE name = '김가상'"), 2);
    assert_eq!(count(&db, &format!("SELECT count(*) FROM instructors WHERE import_id = {} AND distinguisher = '1985년생'", r.import_id)), 1);
}

#[test]
fn 가져오지_않음과_줄_제외와_검토_줄_선택() {
    let db = Db::memory();
    let bytes = xlsx(&[
        row("가", "마술", V::S("2022.3.4"), V::S("2023.2.10")),
        row("가", "바둑", V::S("2022.3.4"), V::E), // 검토
        row("가", "요리", V::S("2023.3.4"), V::E), // 검토
        row("가", "코딩", V::S("2024.3.4"), V::S("2025.2.10")),
        row("나", "마술", V::S("2022.3.4"), V::S("2023.2.10")),
    ]);
    let ex = read_x(&bytes);
    let a = db.read(|c| analyze(c, &ex, today())).unwrap();
    let mut d = new_all(&a);
    d.groups[1].action = Some(GroupAction::Skip); // 나 — 가져오지 않음
    d.review_active = vec![4]; // 바둑만 재직중으로
    d.excluded_rows = vec![6]; // 코딩 줄 제외
    let p = db.read(|c| plan(c, &a, &d)).unwrap();
    assert_eq!(p.outcomes[&3], Outcome::Add);
    assert_eq!(p.outcomes[&4], Outcome::Add);
    assert_eq!(p.outcomes[&5], Outcome::SkipReview);
    assert_eq!(p.outcomes[&6], Outcome::SkipExcluded);
    assert_eq!(p.outcomes[&7], Outcome::SkipGroup);
    assert_eq!(p.summary.careers_added, 2);
    assert_eq!(p.summary.rows_skipped, 3);
    db.write(|c| apply(c, &ex, &d, &p.fingerprint, NOW)).unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM instructors"), 1);
    assert_eq!(count(&db, "SELECT count(*) FROM careers WHERE status = 'ACTIVE' AND end_date IS NULL"), 1, "고른 검토 줄만 재직중");
}

// ---------------- 반영 ----------------

#[test]
fn 미리보기_지문이_없거나_다르면_반영하지_않는다() {
    let db = Db::memory();
    let bytes = xlsx(&[row("가", "마술", V::S("2022.3.4"), V::S("2023.2.10"))]);
    let ex = read_x(&bytes);
    let a = db.read(|c| analyze(c, &ex, today())).unwrap();
    let d = new_all(&a);
    assert_eq!(db.write(|c| apply(c, &ex, &d, "", NOW)).unwrap_err().code, "IMPORT_CHANGED");
    let p = db.read(|c| plan(c, &a, &d)).unwrap();
    // 미리보기 뒤에 같은 이름의 강사가 생기면 결정이 달라야 한다 → 반영하지 않는다
    existing(&db, "가", "다른 사람");
    assert!(db.write(|c| apply(c, &ex, &d, &p.fingerprint, NOW)).is_err());
    assert_eq!(count(&db, "SELECT count(*) FROM imports"), 0);
}

#[test]
fn 반영하면_imports_와_연결되고_파일_이름은_남지_않는다() {
    let db = Db::memory();
    let bytes = xlsx(&[row("가", "마술", V::S("2022.3.4"), V::S("2023.2.10")), row("나", "바둑", V::S("2026.3.4"), V::S("현재"))]);
    let ex = read_x(&bytes);
    let a = db.read(|c| analyze(c, &ex, today())).unwrap();
    let d = new_all(&a);
    let p = db.read(|c| plan(c, &a, &d)).unwrap();
    let r = db.write(|c| apply(c, &ex, &d, &p.fingerprint, NOW)).unwrap();
    let row: (String, String, String, String, i64, i64, i64, i64) = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT label, source_kind, source_sha256, sheet_name, rows_total, instructors_created, careers_added, rows_skipped FROM imports WHERE id = ?1",
                [r.import_id],
                |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?, x.get(3)?, x.get(4)?, x.get(5)?, x.get(6)?, x.get(7)?)),
            )?)
        })
        .unwrap();
    assert_eq!(row.1, "XLSX");
    assert_eq!(row.2, crate::crypto::plain_sha256(&bytes));
    assert_eq!(row.3, "강사명단");
    assert_eq!((row.4, row.5, row.6, row.7), (2, 2, 2, 0));
    assert!(row.0.starts_with("엑셀 가져오기 2026-10-01"));
    assert_eq!(count(&db, &format!("SELECT count(*) FROM careers WHERE import_id = {}", r.import_id)), 2);
    assert_eq!(count(&db, &format!("SELECT count(*) FROM instructors WHERE import_id = {}", r.import_id)), 2);
    // 가져오기 기록은 고치거나 지울 수 없다
    assert!(db.write(|c| Ok(c.execute("DELETE FROM imports", [])?)).is_err());
    // 변경 기록에는 건수만
    let log = db.read(crate::repo::audit::all).unwrap();
    let s = &log.iter().find(|e| e.target_type == "import").unwrap().summary;
    assert!(!s.contains("가") || s.contains("가져오기"), "{s}");
    assert!(!s.contains(FAKE_RRN));
}

#[test]
fn 반영_중_실패하면_아무것도_남지_않는다() {
    let db = Db::memory();
    let bytes = xlsx(&[row("가", "마술", V::S("2022.3.4"), V::S("2023.2.10")), row("나", "바둑", V::S("2022.3.4"), V::S("2023.2.10"))]);
    let ex = read_x(&bytes);
    let a = db.read(|c| analyze(c, &ex, today())).unwrap();
    let d = new_all(&a);
    let p = db.read(|c| plan(c, &a, &d)).unwrap();
    // 두 번째 경력을 넣을 때 실패하게
    db.write(|c| {
        c.execute_batch(
            "CREATE TRIGGER fail_second BEFORE INSERT ON careers
             WHEN (SELECT count(*) FROM careers WHERE import_id IS NOT NULL) >= 1
             BEGIN SELECT RAISE(ABORT, 'test'); END;",
        )?;
        Ok(())
    })
    .unwrap();
    assert!(db.write(|c| apply(c, &ex, &d, &p.fingerprint, NOW)).is_err());
    for t in ["imports", "instructors", "careers"] {
        assert_eq!(count(&db, &format!("SELECT count(*) FROM {t}")), 0, "{t}");
    }
}

#[test]
fn 반영_직전에_before_import_백업을_만든다() {
    let dir = tmp_dir("import-backup");
    let db = Db::open(&dir.join(DB_FILE)).unwrap();
    let bytes = xlsx(&[row("가", "마술", V::S("2022.3.4"), V::S("2023.2.10"))]);
    let ex = read_x(&bytes);
    let a = db.read(|c| analyze(c, &ex, today())).unwrap();
    let d = new_all(&a);
    let p = db.read(|c| plan(c, &a, &d)).unwrap();
    let r = run(&db, &ex, &d, &p.fingerprint, NOW).unwrap();
    assert_eq!(r.summary.careers_added, 1);
    let list = backup::list(&dir.join("backups")).unwrap();
    assert!(list.iter().any(|e| e.kind == backup::Kind::BeforeImport));
}
