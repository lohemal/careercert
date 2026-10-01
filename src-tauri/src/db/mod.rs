//! SQLite 연결 관리.
//!
//! 1인 사용 데스크톱 앱이므로 커넥션 풀 대신 `Mutex<Connection>` 하나로 충분하다.
//! 모든 DB 접근은 `Db::read` / `Db::write` 를 거친다.
//!
//! **`read`/`write` 클로저 안에서 다시 `read`/`write` 를 부르지 말 것** — 같은 Mutex 를
//! 두 번 잡아 멈춘다(학생명단 앱에서 겪은 교착). 안쪽 함수는 `&Connection` 을 받게 만든다.

pub mod backup;
pub mod migrate;
#[cfg(test)]
pub(crate) mod testutil;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

/// `app_meta.app_id` 값. 백업·복원이 "이 프로그램의 자료인가" 를 이 값으로 가린다.
pub const APP_ID: &str = "kr.school.careercert";

/// 자료 폴더 안의 DB 파일 이름.
pub const DB_FILE: &str = "careercert.db";

pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
    /// 복원하고 다시 시작하기 직전에 파일 연결을 놓았다 — 이후 요청은 거절한다
    released: AtomicBool,
}

impl Db {
    /// DB 파일을 열고 필요한 마이그레이션을 적용한다.
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let mut conn = Connection::open(path).map_err(|e| {
            AppError::new(
                "DB_OPEN_FAILED",
                "자료 파일을 열지 못했습니다. 프로그램을 다시 시작해 주세요.",
            )
            .detail(format!("{} :: {e}", path.display()))
        })?;

        setup_conn(&conn)?;
        migrate::run(&mut conn, path)?;

        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
            released: AtomicBool::new(false),
        })
    }

    /// 시험용 메모리 DB (마이그레이션 적용됨).
    #[cfg(test)]
    pub fn memory() -> Self {
        let mut conn = Connection::open_in_memory().expect("메모리 DB를 열지 못했습니다");
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrate::run(&mut conn, Path::new(":memory:")).expect("마이그레이션 실패");
        Self {
            conn: Mutex::new(conn),
            path: PathBuf::from(":memory:"),
            released: AtomicBool::new(false),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn check_open(&self) -> AppResult<()> {
        if self.released.load(Ordering::SeqCst) {
            return Err(AppError::new("DB_RELEASED", "프로그램을 다시 시작하는 중입니다. 잠시 뒤 다시 열어 주세요."));
        }
        Ok(())
    }

    /// 파일 연결을 놓는다(WAL 을 비우고 닫음). 복원 대기 파일을 다음 시작에서 적용하려고 다시 시작하기 직전에만 쓴다.
    pub fn release(&self) -> AppResult<()> {
        let mut guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let _ = guard.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
        self.released.store(true, Ordering::SeqCst);
        let old = std::mem::replace(&mut *guard, Connection::open_in_memory()?);
        old.close().map_err(|(_, e)| AppError::from(e))?;
        Ok(())
    }

    /// 읽기 전용 작업.
    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        self.check_open()?;
        f(&guard)
    }

    /// 쓰기 작업. 클로저가 Err 를 돌려주면 전체가 되돌려진다.
    pub fn write<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        self.check_open()?;
        let tx = guard.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    /// 자료가 성한가. `PRAGMA integrity_check` 가 'ok' 를 주면 성한 것이다.
    pub fn integrity(&self) -> AppResult<String> {
        self.read(|c| Ok(c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?))
    }

    /// 현재 스키마 버전.
    pub fn schema_version(&self) -> AppResult<i32> {
        self.read(|c| Ok(migrate::current_version(c)?))
    }

    /// 백업이 놓이는 폴더 (`<자료 폴더>/backups`).
    pub fn backup_dir(&self) -> PathBuf {
        backup_dir_of(&self.path)
    }

    /// 지금 자료를 통째로 `dest` 에 뜬다 (SQLite 백업 API — WAL 내용 포함).
    pub fn backup_to(&self, dest: &Path) -> AppResult<()> {
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        self.check_open()?;
        copy_with_backup_api(&guard, dest)
    }
}

/// `<DB 가 있는 폴더>/backups`
pub fn backup_dir_of(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
}

/// 열린 연결의 내용을 `dest` 한 파일로 뜬다.
///
/// **파일을 복사하지 않는다.** WAL 모드라 `.db` 만 베끼면 아직 반영되지 않은 내용이 빠진다.
/// 백업본은 `journal_mode=DELETE` 로 바꿔 `-wal` 없이 한 파일로 만든다.
pub fn copy_with_backup_api(conn: &Connection, dest: &Path) -> AppResult<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = Connection::open(dest)?;
    {
        let b = rusqlite::backup::Backup::new(conn, &mut out)?;
        b.run_to_completion(500, std::time::Duration::ZERO, None)?;
    }
    let _: String = out.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get(0))?;
    Ok(())
}

fn setup_conn(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}
