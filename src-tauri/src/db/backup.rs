//! 백업 (Phase 0 기본 — 복원·다른 PC 이전은 Phase 8).
//!
//! 강사 경력과 발급 기록은 이 컴퓨터 안에만 있다. 되돌릴 수 있는 것이 마지막 안전장치다.
//!
//!   * **파일을 그대로 복사하지 않는다.** SQLite 백업 API(`Db::backup_to`)로 한 시점을 뜬다.
//!   * **만들었다고 성공이 아니다.** 만든 파일을 다시 열어 `integrity_check`·이 앱의 자료인지·
//!     구조 버전을 확인하고, 실패하면 지운다. 성하지 않은 백업이 정상 백업을 밀어내면 안 된다.
//!   * **자동 정리는 자동 백업만** 지운다. 수동 백업과 큰 작업 직전의 백업은 남긴다.
//!   * 로그에 자료 내용을 남기지 않는다. 종류·크기만.

use std::path::Path;

use chrono::{DateTime, Local, NaiveDate};
use rusqlite::{Connection, OpenFlags};

use super::{migrate, Db, APP_ID};
use crate::error::{AppError, AppResult};

/// 자동 백업을 몇 개까지 두는가. 하루 한 개이므로 한 달치다.
pub const KEEP_AUTO: usize = 30;

/// 백업을 왜 만들었는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 하루 한 번, 앱을 열 때
    Auto,
    /// 사용자가 [지금 백업] 을 눌러서
    Manual,
    /// 자료 구조를 바꾸기 직전
    BeforeMigration,
    /// 재직중 경력 일괄 종료 직전
    BeforeBulkEnd,
    /// 이름으로 가릴 수 없는 파일
    Other,
}

impl Kind {
    pub fn code(self) -> &'static str {
        match self {
            Kind::Auto => "AUTO",
            Kind::Manual => "MANUAL",
            Kind::BeforeMigration => "BEFORE_MIGRATION",
            Kind::BeforeBulkEnd => "BEFORE_BULK_END",
            Kind::Other => "OTHER",
        }
    }

    /// 파일 이름 앞말.
    fn prefix(self) -> &'static str {
        match self {
            Kind::Auto => "auto_",
            Kind::Manual => "manual_",
            Kind::BeforeMigration => "before_migration_",
            Kind::BeforeBulkEnd => "before_bulk_end_",
            Kind::Other => "",
        }
    }

    /// 자동 정리가 지울 수 있는가 — **자동 백업만.**
    pub fn cleaned_up(self) -> bool {
        self == Kind::Auto
    }

    /// 파일 이름으로 종류를 가린다.
    pub fn of_file(name: &str) -> Kind {
        for k in [Kind::Auto, Kind::Manual, Kind::BeforeMigration, Kind::BeforeBulkEnd] {
            if name.starts_with(k.prefix()) {
                return k;
            }
        }
        Kind::Other
    }
}

/// `20261001_090000`
pub fn stamp(now: DateTime<Local>) -> String {
    now.format("%Y%m%d_%H%M%S").to_string()
}

/// `manual_20261001_090000.db`
pub fn file_name(kind: Kind, now: DateTime<Local>) -> String {
    format!("{}{}.db", kind.prefix(), stamp(now))
}

/// `before_migration_v1_20261001_090000.db` — 어느 구조에서 떴는지 이름에 남긴다.
pub fn file_name_versioned(kind: Kind, version: i32, now: DateTime<Local>) -> String {
    format!("{}v{version}_{}.db", kind.prefix(), stamp(now))
}

// ---------------------------------------------------------------
// 검사
// ---------------------------------------------------------------

/// 백업 파일을 열어 본 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub ok: bool,
    pub schema_version: Option<i32>,
    pub problem: Option<String>,
}

impl Check {
    fn bad(problem: impl Into<String>) -> Self {
        Check {
            ok: false,
            schema_version: None,
            problem: Some(problem.into()),
        }
    }
}

/// 이 파일이 **이 프로그램의 성한 자료**인가.
///
/// 만든 직후에도, 나중에 되돌리기 전에도 같은 검사를 지난다.
pub fn check(path: &Path) -> Check {
    if !path.exists() {
        return Check::bad("파일을 찾을 수 없습니다.");
    }
    let conn = match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(e) => return Check::bad(format!("SQLite 파일로 열지 못했습니다. ({e})")),
    };

    // 1) 파일이 성한가
    match conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)) {
        Ok(v) if v == "ok" => {}
        Ok(v) => return Check::bad(format!("자료가 손상되었습니다. ({v})")),
        Err(e) => return Check::bad(format!("자료를 읽지 못했습니다. ({e})")),
    }

    // 2) 이 프로그램의 자료인가 — 다른 앱의 SQLite 파일을 골라도 거절한다
    let app_id: Option<String> = conn
        .query_row("SELECT value FROM app_meta WHERE key = 'app_id'", [], |r| r.get(0))
        .ok();
    if app_id.as_deref() != Some(APP_ID) {
        return Check::bad("이 프로그램의 자료 파일이 아닙니다.");
    }

    // 3) 이 버전이 읽을 수 있는 구조인가
    let version = match migrate::current_version(&conn) {
        Ok(v) => v,
        Err(e) => return Check::bad(format!("자료 구조 버전을 읽지 못했습니다. ({e})")),
    };
    if version > migrate::latest_version() {
        return Check {
            ok: false,
            schema_version: Some(version),
            problem: Some(format!(
                "더 최신 버전의 프로그램에서 만든 자료입니다(v{version}). 프로그램을 먼저 업데이트해 주세요."
            )),
        };
    }
    if version < 1 {
        return Check {
            ok: false,
            schema_version: Some(version),
            problem: Some("자료 구조가 만들어지기 전의 파일입니다.".into()),
        };
    }

    Check {
        ok: true,
        schema_version: Some(version),
        problem: None,
    }
}

// ---------------------------------------------------------------
// 목록
// ---------------------------------------------------------------

/// 백업 파일 하나.
#[derive(Debug, Clone)]
pub struct Entry {
    pub file_name: String,
    pub path: std::path::PathBuf,
    pub kind: Kind,
}

/// 백업 폴더의 `.db` 파일. 이름 역순 — 같은 종류 안에서는 새 것부터다(이름에 시각이 들어 있다).
pub fn list(dir: &Path) -> AppResult<Vec<Entry>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for item in std::fs::read_dir(dir)? {
        let item = item?;
        let path = item.path();
        if path.extension().and_then(|e| e.to_str()) != Some("db") {
            continue;
        }
        let file_name = item.file_name().to_string_lossy().to_string();
        out.push(Entry {
            kind: Kind::of_file(&file_name),
            path,
            file_name,
        });
    }
    out.sort_by(|a, b| b.file_name.cmp(&a.file_name));
    Ok(out)
}

// ---------------------------------------------------------------
// 만들기
// ---------------------------------------------------------------

/// 백업을 만들고 **바로 확인한다.** 확인에 실패하면 그 파일을 지우고 오류를 낸다.
pub fn create(db: &Db, kind: Kind, now: DateTime<Local>) -> AppResult<Entry> {
    let name = file_name(kind, now);
    let path = db.backup_dir().join(&name);

    db.backup_to(&path)?;

    let result = check(&path);
    if !result.ok {
        let _ = std::fs::remove_file(&path);
        return Err(AppError::new(
            "BACKUP_FAILED",
            format!(
                "백업을 만들었지만 확인에 실패해 지웠습니다. {}",
                result.problem.unwrap_or_default()
            ),
        ));
    }

    log::info!("backup created: {}", kind.code());
    Ok(Entry {
        file_name: name,
        path,
        kind,
    })
}

/// 오늘 만든 **정상** 자동 백업이 있는가.
pub fn has_auto_today(dir: &Path, today: NaiveDate) -> bool {
    let tag = format!("{}{}_", Kind::Auto.prefix(), today.format("%Y%m%d"));
    list(dir)
        .unwrap_or_default()
        .iter()
        .any(|e| e.file_name.starts_with(&tag) && check(&e.path).ok)
}

/// 자동 백업이 무한히 쌓이지 않게 한다. 지운 개수를 돌려준다. **자동 백업만** 지운다.
pub fn cleanup_auto(dir: &Path, keep: usize) -> AppResult<usize> {
    let mut gone = 0;
    for e in list(dir)?
        .into_iter()
        .filter(|e| e.kind.cleaned_up())
        .skip(keep)
    {
        if std::fs::remove_file(&e.path).is_ok() {
            gone += 1;
        }
    }
    if gone > 0 {
        log::info!("backup cleanup: removed {gone} old auto backup(s)");
    }
    Ok(gone)
}

/// 앱을 열 때 하루 한 번. 오늘 것이 이미 있으면 아무것도 하지 않는다.
///
/// 부르는 쪽이 **자료가 성할 때만** 부른다 — 손상된 자료로 뜬 백업이 멀쩡한 지난 백업을
/// 밀어내면 되돌릴 곳이 사라진다.
pub fn auto_if_needed(db: &Db, now: DateTime<Local>) -> AppResult<Option<Entry>> {
    if has_auto_today(&db.backup_dir(), now.date_naive()) {
        return Ok(None);
    }
    let entry = create(db, Kind::Auto, now)?;
    cleanup_auto(&db.backup_dir(), KEEP_AUTO)?;
    Ok(Some(entry))
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod backup_tests;
