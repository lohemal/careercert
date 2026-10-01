use super::*;
use crate::db::testutil::tmp_dir;
use crate::db::{APP_ID, DB_FILE};

fn tables(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn 빈_db에_최신_스키마가_만들어진다() {
    let mut conn = Connection::open_in_memory().unwrap();
    run(&mut conn, Path::new(":memory:")).unwrap();
    assert_eq!(current_version(&conn).unwrap(), latest_version());
    assert!(tables(&conn).contains(&"app_meta".to_string()));
}

#[test]
fn 이_앱의_자료임을_표시한다() {
    let mut conn = Connection::open_in_memory().unwrap();
    run(&mut conn, Path::new(":memory:")).unwrap();
    let id: String = conn
        .query_row("SELECT value FROM app_meta WHERE key='app_id'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(id, APP_ID);
}

#[test]
fn 두_번_실행해도_아무_일도_없다() {
    let mut conn = Connection::open_in_memory().unwrap();
    run(&mut conn, Path::new(":memory:")).unwrap();
    run(&mut conn, Path::new(":memory:")).unwrap();
    assert_eq!(current_version(&conn).unwrap(), latest_version());
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM app_meta WHERE key='app_id'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "두 번째 실행이 시작 자료를 또 넣으면 안 된다");
}

#[test]
fn 더_새로운_자료는_열지_않는다() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "user_version", 999).unwrap();
    let err = run(&mut conn, Path::new(":memory:")).unwrap_err();
    assert_eq!(err.code, "SCHEMA_TOO_NEW");
}

#[test]
fn 마이그레이션_번호는_1부터_빈틈없이_이어진다() {
    for (i, m) in MIGRATIONS.iter().enumerate() {
        assert_eq!(m.version, i as i32 + 1, "{}", m.name);
        assert!(
            m.name.starts_with(&format!("{:03}_", m.version)),
            "파일 이름과 번호가 다르다: {}",
            m.name
        );
    }
}

#[test]
fn 실패한_마이그레이션은_반쯤_적용되지_않는다() {
    let mut conn = Connection::open_in_memory().unwrap();
    let broken = [
        Migration {
            version: 1,
            name: "001_init",
            sql: include_str!("../../migrations/001_init.sql"),
        },
        Migration {
            version: 2,
            name: "002_broken",
            sql: "CREATE TABLE t_ok (id INTEGER) STRICT; THIS IS NOT SQL;",
        },
    ];
    let err = run_list(&mut conn, Path::new(":memory:"), &broken).unwrap_err();
    assert_eq!(err.code, "MIGRATION_FAILED");
    assert_eq!(current_version(&conn).unwrap(), 1, "001 까지만 적용된 상태로 남는다");
    assert!(
        !tables(&conn).contains(&"t_ok".to_string()),
        "실패한 002 의 앞부분도 되돌려져야 한다"
    );
}

#[test]
fn 새_마이그레이션은_기존_자료를_지우지_않고_직전에_백업한다() {
    let dir = tmp_dir("migrate-next");
    let path = dir.join(DB_FILE);
    {
        let mut conn = Connection::open(&path).unwrap();
        run(&mut conn, &path).unwrap();
        conn.execute("INSERT INTO app_meta(key, value) VALUES ('probe', '가상 값')", [])
            .unwrap();
    }

    // 다음 판에서 002 가 늘었다고 치고 적용해 본다
    let next = [
        Migration {
            version: 1,
            name: "001_init",
            sql: include_str!("../../migrations/001_init.sql"),
        },
        Migration {
            version: 2,
            name: "002_test_only",
            sql: "CREATE TABLE probe_next (id INTEGER PRIMARY KEY) STRICT;",
        },
    ];
    let mut conn = Connection::open(&path).unwrap();
    run_list(&mut conn, &path, &next).unwrap();

    assert_eq!(current_version(&conn).unwrap(), 2);
    let v: String = conn
        .query_row("SELECT value FROM app_meta WHERE key='probe'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, "가상 값");

    let names: Vec<String> = std::fs::read_dir(dir.join("backups"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n.starts_with("before_migration_v1_")),
        "{names:?}"
    );
}

#[test]
fn 이미_최신이면_백업도_만들지_않는다() {
    let dir = tmp_dir("migrate-same");
    let path = dir.join(DB_FILE);
    let mut conn = Connection::open(&path).unwrap();
    run(&mut conn, &path).unwrap();
    run(&mut conn, &path).unwrap();
    assert!(!dir.join("backups").exists());
}
