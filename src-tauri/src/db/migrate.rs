//! 마이그레이션 러너.
//!
//! `PRAGMA user_version` 을 스키마 버전으로 쓰고, 아래 목록을 번호순으로 적용한다.
//! 기존 자료가 있으면 적용 **직전에** `backups/before_migration_v{N}_*.db` 로 떠 둔다.
//!
//! 새 마이그레이션 더하는 법
//!   1. `migrations/00N_설명.sql` 파일을 만든다
//!   2. 아래 MIGRATIONS 에 한 줄 더한다
//!
//! 배포된 뒤에는 이미 나간 파일을 고치지 않는다 — 그 구조로 자료를 만든 사용자가 있다.

use std::path::Path;

use rusqlite::Connection;

use super::backup::{self, Kind};
use super::{backup_dir_of, copy_with_backup_api};
use crate::error::{AppError, AppResult};

pub(crate) struct Migration {
    pub version: i32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "001_init",
        sql: include_str!("../../migrations/001_init.sql"),
    },
    Migration {
        version: 2,
        name: "002_school_settings",
        sql: include_str!("../../migrations/002_school_settings.sql"),
    },
    Migration {
        version: 3,
        name: "003_instructors_careers",
        sql: include_str!("../../migrations/003_instructors_careers.sql"),
    },
    Migration {
        version: 4,
        name: "004_audit_log",
        sql: include_str!("../../migrations/004_audit_log.sql"),
    },
    Migration {
        version: 5,
        name: "005_planned_end_date",
        sql: include_str!("../../migrations/005_planned_end_date.sql"),
    },
    Migration {
        version: 6,
        name: "006_certificates",
        sql: include_str!("../../migrations/006_certificates.sql"),
    },
    Migration {
        version: 7,
        name: "007_recovery_imports",
        sql: include_str!("../../migrations/007_recovery_imports.sql"),
    },
    Migration {
        version: 8,
        name: "008_school_logo",
        sql: include_str!("../../migrations/008_school_logo.sql"),
    },
    Migration {
        version: 9,
        name: "009_discard_overlap_ack",
        sql: include_str!("../../migrations/009_discard_overlap_ack.sql"),
    },
];

pub fn latest_version() -> i32 {
    MIGRATIONS.iter().map(|m| m.version).max().unwrap_or(0)
}

pub fn current_version(conn: &Connection) -> rusqlite::Result<i32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

pub fn run(conn: &mut Connection, db_path: &Path) -> AppResult<()> {
    run_list(conn, db_path, MIGRATIONS)
}

/// 목록을 받아 적용한다. 시험에서 앞으로 더할 마이그레이션을 흉내 내는 데 쓴다.
pub(crate) fn run_list(conn: &mut Connection, db_path: &Path, list: &[Migration]) -> AppResult<()> {
    let from = current_version(conn)?;
    let to = list.iter().map(|m| m.version).max().unwrap_or(0);

    if from == to {
        return Ok(());
    }
    if from > to {
        return Err(AppError::new(
            "SCHEMA_TOO_NEW",
            "더 최신 버전의 프로그램에서 만든 자료입니다. 프로그램을 최신 버전으로 업데이트해 주세요.",
        )
        .detail(format!("db user_version={from}, app supports={to}")));
    }

    // 기존 자료가 있을 때만 백업 (처음 만들 때는 뜰 것이 없다)
    if from > 0 && db_path.exists() {
        let dest = backup_dir_of(db_path).join(backup::file_name_versioned(
            Kind::BeforeMigration,
            from,
            chrono::Local::now(),
        ));
        copy_with_backup_api(conn, &dest)?;
        log::info!("backup before migration v{from}");
    }

    for m in list.iter().filter(|m| m.version > from) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql).map_err(|e| {
            AppError::new(
                "MIGRATION_FAILED",
                "자료 구조를 업데이트하지 못했습니다. 프로그램을 다시 시작해 주세요.",
            )
            .detail(format!("{} :: {e}", m.name))
        })?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
        log::info!("migration applied: {} (v{})", m.name, m.version);
    }

    Ok(())
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod migrate_tests;
