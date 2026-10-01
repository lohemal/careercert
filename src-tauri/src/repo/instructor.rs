//! `instructors` — SQL 만. 검사는 `domain::instructor::prepare`, 절차는 `service::instructor`.

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::domain::instructor::{Instructor, InstructorFields};
use crate::error::{AppError, AppResult};

const COLS: &str = "id, uuid, name, distinguisher, phone, memo, archived_at, created_at, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Instructor> {
    Ok(Instructor {
        id: r.get(0)?,
        uuid: r.get(1)?,
        name: r.get(2)?,
        distinguisher: r.get(3)?,
        phone: r.get(4)?,
        memo: r.get(5)?,
        archived_at: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
    })
}

/// 새 강사. uuid 는 여기서 만든다. 새 번호를 돌려준다.
pub fn insert(conn: &Connection, f: &InstructorFields, now: &str) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO instructors (uuid, name, distinguisher, phone, memo, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![uuid::Uuid::new_v4().to_string(), f.name, f.distinguisher, f.phone, f.memo, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, f: &InstructorFields, now: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE instructors
            SET name = ?2, distinguisher = ?3, phone = ?4, memo = ?5, updated_at = ?6
          WHERE id = ?1",
        params![id, f.name, f.distinguisher, f.phone, f.memo, now],
    )?;
    expect_one(n)
}

/// 보관(`Some(시각)`) 또는 보관 해제(`None`).
pub fn set_archived(conn: &Connection, id: i64, archived_at: Option<&str>, now: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE instructors SET archived_at = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, archived_at, now],
    )?;
    expect_one(n)
}

/// 보관 여부와 상관없이 한 명.
pub fn find(conn: &Connection, id: i64) -> AppResult<Option<Instructor>> {
    Ok(conn
        .query_row(&format!("SELECT {COLS} FROM instructors WHERE id = ?1"), [id], from_row)
        .optional()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Instructor> {
    find(conn, id)?.ok_or_else(not_found)
}

/// 어떤 강사를 보일까. 화면의 필터 [재직중 · 전체 · 보관] 과 같다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Scope {
    /// 재직중 경력이 하나라도 있는 강사 (보관 제외)
    Active,
    /// 보관하지 않은 강사 전부
    #[default]
    All,
    /// 보관한 강사만
    Archived,
    /// 보관 여부와 상관없이 전부 (화면 필터에는 없다)
    Everything,
}

impl Scope {
    fn code(self) -> &'static str {
        match self {
            Scope::Active => "ACTIVE",
            Scope::All => "ALL",
            Scope::Archived => "ARCHIVED",
            Scope::Everything => "EVERYTHING",
        }
    }
}

/// 목록 조건.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// 이름·구분 메모에 들어 있는 글자. 비우면 전체.
    pub query: String,
    pub scope: Scope,
}

/// 목록 한 줄 — 강사와 경력 수.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub instructor: Instructor,
    /// 보관하지 않은 경력 수
    pub careers: i64,
    /// 그중 재직중
    pub active_careers: i64,
    /// 보관하지 않은 경력의 프로그램명 (가나다순, 겹치지 않게) — 동명이인을 가리는 데 쓴다
    pub programs: Vec<String>,
}

/// 찾기. 이름 차례, 같은 이름은 등록 차례.
///
/// `LIKE` 대신 `instr` 로 찾는다 — 검색어의 `%`·`_` 가 와일드카드로 바뀌지 않는다.
pub fn search(conn: &Connection, filter: &Filter) -> AppResult<Vec<Summary>> {
    let q = filter.query.trim();
    let sql = format!(
        "SELECT * FROM (
            SELECT {cols},
                   (SELECT COUNT(*) FROM careers c WHERE c.instructor_id = i.id AND c.archived_at IS NULL) AS n_all,
                   (SELECT COUNT(*) FROM careers c WHERE c.instructor_id = i.id AND c.archived_at IS NULL
                                                     AND c.status = 'ACTIVE') AS n_active,
                   (SELECT group_concat(p, char(31)) FROM (
                        SELECT DISTINCT program_name AS p FROM careers c
                         WHERE c.instructor_id = i.id AND c.archived_at IS NULL ORDER BY p)) AS programs
              FROM instructors i
             WHERE (?1 = '' OR instr(i.name, ?1) > 0 OR instr(i.distinguisher, ?1) > 0)
         )
          WHERE CASE ?2
                  WHEN 'ACTIVE'   THEN archived_at IS NULL AND n_active > 0
                  WHEN 'ALL'      THEN archived_at IS NULL
                  WHEN 'ARCHIVED' THEN archived_at IS NOT NULL
                  ELSE 1
                END
          ORDER BY name, id",
        cols = COLS
            .split(", ")
            .map(|c| format!("i.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![q, filter.scope.code()], |r| {
            Ok(Summary {
                instructor: from_row(r)?,
                careers: r.get(9)?,
                active_careers: r.get(10)?,
                programs: r
                    .get::<_, Option<String>>(11)?
                    .map(|s| s.split('\u{1f}').map(String::from).collect())
                    .unwrap_or_default(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn not_found() -> AppError {
    AppError::not_found("강사를 찾을 수 없습니다.")
}

fn expect_one(n: usize) -> AppResult<()> {
    if n == 1 {
        Ok(())
    } else {
        Err(not_found())
    }
}
