//! DB 제약 시험 — domain 을 **건너뛰고** SQL 을 직접 넣어, 화면·규칙을 거치지 않은 값도
//! DB 가 막는지 본다(예: 나중에 가져오기 코드가 실수해도).

use rusqlite::{params, Connection};

use crate::db::Db;

const NOW: &str = "2026-10-01T09:00:00";

fn instructor(c: &Connection, name: &str) -> i64 {
    c.execute(
        "INSERT INTO instructors (uuid, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![uuid::Uuid::new_v4().to_string(), name, NOW],
    )
    .unwrap();
    c.last_insert_rowid()
}

/// 경력 한 줄을 SQL 로 바로 넣는다.
fn raw(c: &Connection, start: &str, end: Option<&str>, status: &str, reason: Option<&str>) -> rusqlite::Result<usize> {
    let who = instructor(c, "가상강사");
    c.execute(
        "INSERT INTO careers (uuid, instructor_id, program_name, position, duty, start_date,
                              end_date, status, end_reason, created_at, updated_at)
         VALUES (?1, ?2, '마술', '강사', '방과후학교 마술', ?3, ?4, ?5, ?6, ?7, ?7)",
        params![uuid::Uuid::new_v4().to_string(), who, start, end, status, reason, NOW],
    )
}

fn check_failed(r: rusqlite::Result<usize>) -> bool {
    matches!(r, Err(rusqlite::Error::SqliteFailure(_, Some(ref m))) if m.contains("CHECK constraint failed"))
}

#[test]
fn db_재직중이고_종료일_없음은_받는다() {
    let db = Db::memory();
    db.read(|c| {
        assert!(raw(c, "2026-03-04", None, "ACTIVE", None).is_ok());
        Ok(())
    })
    .unwrap();
}

#[test]
fn db_종료일과_사유가_있는_종료는_받는다() {
    let db = Db::memory();
    db.read(|c| {
        assert!(raw(c, "2025-03-05", Some("2026-02-06"), "ENDED", Some("CONTRACT_END")).is_ok());
        assert!(raw(c, "2025-03-05", Some("2025-03-05"), "ENDED", Some("TERMINATED")).is_ok());
        Ok(())
    })
    .unwrap();
}

#[test]
fn db_어긋난_상태와_날짜는_거부한다() {
    let db = Db::memory();
    db.read(|c| {
        let cases: [(&str, Option<&str>, &str, Option<&str>, &str); 6] = [
            ("2026-03-04", Some("2026-10-31"), "ACTIVE", None, "재직중 + 종료일"),
            ("2026-03-04", None, "ACTIVE", Some("CONTRACT_END"), "재직중 + 종료 사유"),
            ("2026-03-04", None, "ENDED", Some("CONTRACT_END"), "종료 + 종료일 없음"),
            ("2026-03-04", Some("2026-10-31"), "ENDED", None, "종료 + 사유 없음"),
            ("2026-03-04", Some("2026-03-03"), "ENDED", Some("TERMINATED"), "종료일 < 시작일"),
            ("2026-03-04", None, "PAUSED", None, "없는 상태"),
        ];
        for (start, end, status, reason, why) in cases {
            assert!(check_failed(raw(c, start, end, status, reason)), "{why}");
        }
        assert!(
            check_failed(raw(c, "2026-03-04", Some("2026-10-31"), "ENDED", Some("RETIRED"))),
            "없는 종료 사유"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn db_날짜_칸에_현재나_엉터리_날짜는_들어가지_않는다() {
    let db = Db::memory();
    db.read(|c| {
        for (start, end, status, reason) in [
            ("2026-03-04", Some("현재"), "ENDED", Some("CONTRACT_END")),
            ("현재", None, "ACTIVE", None),
            ("2026-02-30", None, "ACTIVE", None),
            ("2026-13-01", None, "ACTIVE", None),
            ("2026-3-4", None, "ACTIVE", None),
            ("2026.03.04", None, "ACTIVE", None),
            ("", None, "ACTIVE", None),
        ] {
            assert!(check_failed(raw(c, start, end, status, reason)), "{start} / {end:?}");
        }
        let n: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM careers WHERE start_date = '현재' OR end_date = '현재'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn db_빈_프로그램명이나_앞뒤_공백은_거부한다() {
    let db = Db::memory();
    db.read(|c| {
        let who = instructor(c, "가상강사");
        for program in ["", " 마술"] {
            let r = c.execute(
                "INSERT INTO careers (uuid, instructor_id, program_name, position, duty, start_date,
                                      status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, '강사', '방과후학교', '2026-03-04', 'ACTIVE', ?4, ?4)",
                params![uuid::Uuid::new_v4().to_string(), who, program, NOW],
            );
            assert!(check_failed(r), "{program:?}");
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn db_없는_강사의_경력은_넣을_수_없다() {
    let db = Db::memory();
    db.read(|c| {
        let r = c.execute(
            "INSERT INTO careers (uuid, instructor_id, program_name, position, duty, start_date,
                                  status, created_at, updated_at)
             VALUES (?1, 999, '마술', '강사', '방과후학교 마술', '2026-03-04', 'ACTIVE', ?2, ?2)",
            params![uuid::Uuid::new_v4().to_string(), NOW],
        );
        assert!(matches!(r, Err(rusqlite::Error::SqliteFailure(_, Some(ref m))) if m.contains("FOREIGN KEY")));
        Ok(())
    })
    .unwrap();
}

// ---------------- 강사 표 ----------------

#[test]
fn 강사_표에는_주민번호와_주소_칸이_없다() {
    let db = Db::memory();
    let cols: Vec<String> = db
        .read(|c| {
            Ok(c.prepare("SELECT name FROM pragma_table_info('instructors') ORDER BY cid")?
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?)
        })
        .unwrap();
    // 열을 더하려면 이 목록을 고쳐야 한다 — 그때 개인정보 칸인지 한 번 더 따져 보라는 뜻
    assert_eq!(
        cols,
        vec![
            "id",
            "uuid",
            "name",
            "distinguisher",
            "phone",
            "memo",
            "archived_at",
            "created_at",
            "updated_at"
        ]
    );
    for c in &cols {
        let l = c.to_lowercase();
        for bad in ["rrn", "resident", "jumin", "ssn", "addr", "주민", "주소"] {
            assert!(!l.contains(bad), "{c}");
        }
    }
}

#[test]
fn 같은_이름의_강사를_둘_넣을_수_있다() {
    let db = Db::memory();
    db.read(|c| {
        let a = instructor(c, "김으뜸");
        let b = instructor(c, "김으뜸");
        assert_ne!(a, b);
        Ok(())
    })
    .unwrap();
}

#[test]
fn 강사_uuid는_겹칠_수_없다() {
    let db = Db::memory();
    db.read(|c| {
        let u = uuid::Uuid::new_v4().to_string();
        let ins = |name: &str| {
            c.execute(
                "INSERT INTO instructors (uuid, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
                params![u, name, NOW],
            )
        };
        assert!(ins("가").is_ok());
        let r = ins("나");
        assert!(matches!(r, Err(rusqlite::Error::SqliteFailure(_, Some(ref m))) if m.contains("UNIQUE")));
        Ok(())
    })
    .unwrap();
}

#[test]
fn db_예정_종료일은_시작일보다_빠르거나_엉터리일_수_없다() {
    let db = Db::memory();
    db.read(|c| {
        raw(c, "2026-03-04", None, "ACTIVE", None).unwrap();
        let id = c.last_insert_rowid();
        let set = |v: Option<&str>| {
            c.execute("UPDATE careers SET planned_end_date = ?1 WHERE id = ?2", params![v, id])
        };
        assert!(set(Some("2027-02-05")).is_ok());
        assert!(set(None).is_ok());
        assert!(set(Some("2026-03-04")).is_ok(), "시작일과 같은 날은 받는다");
        for bad in ["2026-03-03", "2027-02-30", "현재", "2027.02.05"] {
            assert!(check_failed(set(Some(bad))), "{bad}");
        }
        Ok(())
    })
    .unwrap();
}
