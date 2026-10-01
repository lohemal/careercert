use chrono::TimeZone;

use super::*;
use crate::db::testutil::tmp_dir;
use crate::db::DB_FILE;

fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Local> {
    Local.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
}

fn open_db(tag: &str) -> (std::path::PathBuf, Db) {
    let dir = tmp_dir(tag);
    let db = Db::open(&dir.join(DB_FILE)).unwrap();
    (dir, db)
}

#[test]
fn 백업은_검사를_통과하는_한_파일이다() {
    let (_dir, db) = open_db("backup-basic");
    let e = create(&db, Kind::Manual, at(2026, 10, 1, 9, 0)).unwrap();
    assert_eq!(e.file_name, "manual_20261001_090000.db");
    assert_eq!(
        check(&e.path),
        Check {
            ok: true,
            schema_version: Some(migrate::latest_version()),
            problem: None
        }
    );
    // WAL 을 끈 한 파일 — 옆에 -wal 이 남지 않는다
    let mut wal = e.path.as_os_str().to_os_string();
    wal.push("-wal");
    assert!(!std::path::PathBuf::from(wal).exists());
}

#[test]
fn 방금_쓴_내용도_백업에_들어간다() {
    // WAL 모드에서 파일만 베끼면 빠지는 내용이다
    let (_dir, db) = open_db("backup-wal");
    db.write(|c| {
        c.execute("INSERT INTO app_meta(key, value) VALUES ('probe', '방금 쓴 값')", [])?;
        Ok(())
    })
    .unwrap();
    let e = create(&db, Kind::Manual, at(2026, 10, 1, 9, 5)).unwrap();

    let copy = Connection::open(&e.path).unwrap();
    let v: String = copy
        .query_row("SELECT value FROM app_meta WHERE key='probe'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, "방금 쓴 값");
}

#[test]
fn 다른_프로그램의_sqlite_파일은_거절한다() {
    let dir = tmp_dir("backup-foreign");
    let path = dir.join("other.db");
    let c = Connection::open(&path).unwrap();
    c.execute_batch("CREATE TABLE students(id INTEGER); PRAGMA user_version = 1;")
        .unwrap();
    drop(c);
    let r = check(&path);
    assert!(!r.ok);
    assert_eq!(r.problem.as_deref(), Some("이 프로그램의 자료 파일이 아닙니다."));
}

#[test]
fn 깨진_파일과_없는_파일은_거절한다() {
    let dir = tmp_dir("backup-broken");
    let path = dir.join("broken.db");
    std::fs::write(&path, b"this is not a sqlite database at all").unwrap();
    assert!(!check(&path).ok);
    assert!(!check(&dir.join("none.db")).ok);
}

#[test]
fn 더_새로운_구조의_백업은_거절한다() {
    let (_dir, db) = open_db("backup-newer");
    let e = create(&db, Kind::Manual, at(2026, 10, 1, 9, 10)).unwrap();
    Connection::open(&e.path)
        .unwrap()
        .pragma_update(None, "user_version", 999)
        .unwrap();
    let r = check(&e.path);
    assert!(!r.ok);
    assert_eq!(r.schema_version, Some(999));
}

#[test]
fn 자동_백업은_하루_한_번만_만든다() {
    let (_dir, db) = open_db("backup-auto");
    assert!(auto_if_needed(&db, at(2026, 10, 1, 8, 0)).unwrap().is_some());
    assert!(auto_if_needed(&db, at(2026, 10, 1, 17, 0)).unwrap().is_none());
    assert!(auto_if_needed(&db, at(2026, 10, 2, 8, 0)).unwrap().is_some());
    assert_eq!(list(&db.backup_dir()).unwrap().len(), 2);
}

#[test]
fn 오늘_자동_백업이_깨져_있으면_다시_만든다() {
    let (_dir, db) = open_db("backup-auto-broken");
    let e = auto_if_needed(&db, at(2026, 10, 1, 8, 0)).unwrap().unwrap();
    std::fs::write(&e.path, b"broken").unwrap();
    assert!(auto_if_needed(&db, at(2026, 10, 1, 9, 0)).unwrap().is_some());
}

#[test]
fn 자동_정리는_자동_백업만_지운다() {
    let (_dir, db) = open_db("backup-cleanup");
    create(&db, Kind::Manual, at(2026, 9, 1, 9, 0)).unwrap();
    for d in 1..=5 {
        create(&db, Kind::Auto, at(2026, 9, d, 8, 0)).unwrap();
    }
    let gone = cleanup_auto(&db.backup_dir(), 3).unwrap();
    assert_eq!(gone, 2);

    let names: Vec<String> = list(&db.backup_dir())
        .unwrap()
        .into_iter()
        .map(|e| e.file_name)
        .collect();
    assert_eq!(
        names,
        vec![
            "manual_20260901_090000.db",
            "auto_20260905_080000.db",
            "auto_20260904_080000.db",
            "auto_20260903_080000.db",
        ],
        "가장 오래된 자동 백업 둘만 지워지고 수동 백업은 남는다"
    );
}

#[test]
fn 파일_이름으로_종류를_가린다() {
    assert_eq!(Kind::of_file("auto_20261001_080000.db"), Kind::Auto);
    assert_eq!(Kind::of_file("manual_20261001_080000.db"), Kind::Manual);
    assert_eq!(
        Kind::of_file("before_migration_v1_20261001_080000.db"),
        Kind::BeforeMigration
    );
    assert_eq!(
        Kind::of_file("before_bulk_end_20261001_080000.db"),
        Kind::BeforeBulkEnd
    );
    assert!(!Kind::BeforeBulkEnd.cleaned_up(), "큰 작업 직전 백업은 자동 정리하지 않는다");
    assert_eq!(Kind::of_file("careercert.db"), Kind::Other);
}
