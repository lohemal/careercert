//! `careers` — SQL 만. 검사는 `domain::career`, 절차는 `service::career`.

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::domain::career::{Career, CareerFields, Term};
use crate::domain::date;
use crate::error::{AppError, AppResult};

const COLS: &str = "id, uuid, instructor_id, program_name, position, duty, start_date, end_date,
                    status, end_reason, import_id, memo, archived_at, created_at, updated_at,
                    planned_end_date";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Career> {
    let start: String = r.get(6)?;
    let end: Option<String> = r.get(7)?;
    let status: String = r.get(8)?;
    let reason: Option<String> = r.get(9)?;
    // DB CHECK 가 막으므로 정상이라면 아래 두 오류는 나올 수 없다
    let bad = |what: &str| {
        rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Text,
            format!("careers 행의 {what} 를 읽지 못했습니다").into(),
        )
    };
    let start_date = date::parse_iso(&start).map_err(|_| bad("시작일"))?;
    let planned: Option<String> = r.get(15)?;
    let planned_end_date = match planned {
        None => None,
        Some(p) => Some(date::parse_iso(&p).map_err(|_| bad("예정 종료일"))?),
    };
    let term = Term::from_columns(&status, end.as_deref(), reason.as_deref()).ok_or_else(|| bad("상태"))?;
    Ok(Career {
        id: r.get(0)?,
        uuid: r.get(1)?,
        instructor_id: r.get(2)?,
        fields: CareerFields {
            program_name: r.get(3)?,
            position: r.get(4)?,
            duty: r.get(5)?,
            start_date,
            term,
            planned_end_date,
            memo: r.get(11)?,
        },
        import_id: r.get(10)?,
        archived_at: r.get(12)?,
        created_at: r.get(13)?,
        updated_at: r.get(14)?,
    })
}

/// (status, end_date, end_reason) 세 칸.
fn term_cols(t: &Term) -> (&'static str, Option<String>, Option<&'static str>) {
    (
        t.status().code(),
        t.end_date().map(date::to_iso),
        t.reason().map(|r| r.code()),
    )
}

/// 새 경력. uuid 는 여기서 만든다. 새 번호를 돌려준다.
pub fn insert(
    conn: &Connection,
    instructor_id: i64,
    f: &CareerFields,
    import_id: Option<i64>,
    now: &str,
) -> AppResult<i64> {
    let (status, end_date, reason) = term_cols(&f.term);
    conn.execute(
        "INSERT INTO careers (uuid, instructor_id, program_name, position, duty, start_date,
                              end_date, status, end_reason, import_id, memo, created_at, updated_at,
                              planned_end_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, ?13)",
        params![
            uuid::Uuid::new_v4().to_string(),
            instructor_id,
            f.program_name,
            f.position,
            f.duty,
            date::to_iso(f.start_date),
            end_date,
            status,
            reason,
            import_id,
            f.memo,
            now,
            f.planned_end_date.map(date::to_iso),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 내용 전체를 바꾼다 (상태 포함). 강사·가져오기 연결은 바꾸지 않는다.
pub fn update(conn: &Connection, id: i64, f: &CareerFields, now: &str) -> AppResult<()> {
    let (status, end_date, reason) = term_cols(&f.term);
    let n = conn.execute(
        "UPDATE careers
            SET program_name = ?2, position = ?3, duty = ?4, start_date = ?5,
                end_date = ?6, status = ?7, end_reason = ?8, memo = ?9, updated_at = ?10,
                planned_end_date = ?11
          WHERE id = ?1",
        params![
            id,
            f.program_name,
            f.position,
            f.duty,
            date::to_iso(f.start_date),
            end_date,
            status,
            reason,
            f.memo,
            now,
            f.planned_end_date.map(date::to_iso),
        ],
    )?;
    expect_one(n)
}

/// 상태·종료일·종료 사유만 바꾼다 (종료 처리).
pub fn set_term(conn: &Connection, id: i64, t: &Term, now: &str) -> AppResult<()> {
    let (status, end_date, reason) = term_cols(t);
    let n = conn.execute(
        "UPDATE careers SET status = ?2, end_date = ?3, end_reason = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, status, end_date, reason, now],
    )?;
    expect_one(n)
}

/// 보관(`Some(시각)`) 또는 보관 해제(`None`).
pub fn set_archived(conn: &Connection, id: i64, archived_at: Option<&str>, now: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE careers SET archived_at = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, archived_at, now],
    )?;
    expect_one(n)
}

/// 보관 여부와 상관없이 하나.
pub fn find(conn: &Connection, id: i64) -> AppResult<Option<Career>> {
    Ok(conn
        .query_row(&format!("SELECT {COLS} FROM careers WHERE id = ?1"), [id], from_row)
        .optional()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Career> {
    find(conn, id)?.ok_or_else(not_found)
}

/// 한 강사의 경력. 시작일 차례(같으면 등록 차례). 보관한 경력은 기본으로 숨긴다.
pub fn list_by_instructor(conn: &Connection, instructor_id: i64, include_archived: bool) -> AppResult<Vec<Career>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM careers
          WHERE instructor_id = ?1 AND (?2 OR archived_at IS NULL)
          ORDER BY start_date, id"
    ))?;
    let rows = stmt
        .query_map(params![instructor_id, include_archived], from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn not_found() -> AppError {
    AppError::not_found("경력을 찾을 수 없습니다.")
}

fn expect_one(n: usize) -> AppResult<()> {
    if n == 1 {
        Ok(())
    } else {
        Err(not_found())
    }
}

#[cfg(test)]
#[path = "career_tests.rs"]
mod career_tests;
