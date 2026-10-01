//! `certificates` · `certificate_items` · `certificate_outputs` — SQL 만.
//!
//! 고치는 함수는 `void` 하나뿐이다(ISSUED → VOIDED). 나머지 변경은 DB 트리거가 거부한다.
//! 주민번호·주소는 암호문(`sensitive_*`)으로만 오간다 — 이 파일에는 평문이 지나가지 않는다.

use rusqlite::{params, Connection, OptionalExtension, Row as SqlRow};

use crate::error::{AppError, AppResult};

/// 새 발급 기록 (모든 값은 service 가 만든다).
pub struct NewCertificate<'a> {
    pub uuid: &'a str,
    pub issue_no: &'a str,
    pub issue_no_key: &'a str,
    pub issued_on: &'a str,
    pub template_version: u32,
    pub title: &'a str,
    pub purpose: &'a str,
    pub holder_name: &'a str,
    pub sensitive_nonce: &'a [u8],
    pub sensitive_cipher: &'a [u8],
    pub key_id: &'a str,
    pub masked_rrn: &'a str,
    pub rrn_display: &'a str,
    pub issuer_title: &'a str,
    pub department: &'a str,
    pub manager_name: &'a str,
    pub phone: &'a str,
    pub item_count: usize,
    pub doc_hash: &'a str,
    pub source_instructor_id: Option<i64>,
    pub copied_from_certificate_id: Option<i64>,
    pub created_at: &'a str,
    pub created_by: &'a str,
}

pub fn insert(conn: &Connection, c: &NewCertificate<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO certificates (uuid, issue_no, issue_no_key, issued_on, template_version, title, purpose,
                                   holder_name, sensitive_nonce, sensitive_cipher, key_id, masked_rrn, rrn_display,
                                   issuer_title, department, manager_name, phone, item_count, doc_hash,
                                   source_instructor_id, copied_from_certificate_id, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
        params![
            c.uuid,
            c.issue_no,
            c.issue_no_key,
            c.issued_on,
            c.template_version,
            c.title,
            c.purpose,
            c.holder_name,
            c.sensitive_nonce,
            c.sensitive_cipher,
            c.key_id,
            c.masked_rrn,
            c.rrn_display,
            c.issuer_title,
            c.department,
            c.manager_name,
            c.phone,
            c.item_count as i64,
            c.doc_hash,
            c.source_instructor_id,
            c.copied_from_certificate_id,
            c.created_at,
            c.created_by,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 경력 한 줄 (발급 당시 복사본).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    pub seq: i64,
    pub source_career_id: Option<i64>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub from_text: String,
    pub to_text: String,
    pub program_name: String,
    pub position: String,
    pub duty: String,
}

pub fn insert_item(conn: &Connection, certificate_id: i64, it: &ItemRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO certificate_items (certificate_id, seq, source_career_id, start_date, end_date,
                                        from_text, to_text, program_name, position, duty)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            certificate_id,
            it.seq,
            it.source_career_id,
            it.start_date,
            it.end_date,
            it.from_text,
            it.to_text,
            it.program_name,
            it.position,
            it.duty
        ],
    )?;
    Ok(())
}

/// 발급 기록 한 줄 (암호문 그대로).
#[derive(Clone, PartialEq, Eq)]
pub struct Row {
    pub id: i64,
    pub uuid: String,
    pub issue_no: String,
    pub issued_on: String,
    pub template_version: u32,
    pub title: String,
    pub purpose: String,
    pub holder_name: String,
    pub sensitive_nonce: Vec<u8>,
    pub sensitive_cipher: Vec<u8>,
    pub key_id: String,
    pub masked_rrn: String,
    pub rrn_display: String,
    pub issuer_title: String,
    pub department: String,
    pub manager_name: String,
    pub phone: String,
    pub item_count: i64,
    pub doc_hash: String,
    pub status: String,
    pub voided_at: Option<String>,
    pub void_reason: Option<String>,
    pub source_instructor_id: Option<i64>,
    pub copied_from_certificate_id: Option<i64>,
    pub created_at: String,
    pub created_by: String,
}

impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("certificates::Row")
            .field("id", &self.id)
            .field("issue_no", &self.issue_no)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

const COLS: &str = "id, uuid, issue_no, issued_on, template_version, title, purpose, holder_name,
                    sensitive_nonce, sensitive_cipher, key_id, masked_rrn, rrn_display, issuer_title,
                    department, manager_name, phone, item_count, doc_hash, status, voided_at, void_reason,
                    source_instructor_id, copied_from_certificate_id, created_at, created_by";

fn from_row(r: &SqlRow<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        uuid: r.get(1)?,
        issue_no: r.get(2)?,
        issued_on: r.get(3)?,
        template_version: r.get(4)?,
        title: r.get(5)?,
        purpose: r.get(6)?,
        holder_name: r.get(7)?,
        sensitive_nonce: r.get(8)?,
        sensitive_cipher: r.get(9)?,
        key_id: r.get(10)?,
        masked_rrn: r.get(11)?,
        rrn_display: r.get(12)?,
        issuer_title: r.get(13)?,
        department: r.get(14)?,
        manager_name: r.get(15)?,
        phone: r.get(16)?,
        item_count: r.get(17)?,
        doc_hash: r.get(18)?,
        status: r.get(19)?,
        voided_at: r.get(20)?,
        void_reason: r.get(21)?,
        source_instructor_id: r.get(22)?,
        copied_from_certificate_id: r.get(23)?,
        created_at: r.get(24)?,
        created_by: r.get(25)?,
    })
}

pub fn find(conn: &Connection, id: i64) -> AppResult<Option<Row>> {
    Ok(conn
        .query_row(&format!("SELECT {COLS} FROM certificates WHERE id = ?1"), [id], from_row)
        .optional()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Row> {
    find(conn, id)?.ok_or_else(|| AppError::not_found("발급 기록을 찾을 수 없습니다."))
}

pub fn items(conn: &Connection, certificate_id: i64) -> AppResult<Vec<ItemRow>> {
    let mut stmt = conn.prepare(
        "SELECT seq, source_career_id, start_date, end_date, from_text, to_text, program_name, position, duty
           FROM certificate_items WHERE certificate_id = ?1 ORDER BY seq",
    )?;
    let rows = stmt
        .query_map([certificate_id], |r| {
            Ok(ItemRow {
                seq: r.get(0)?,
                source_career_id: r.get(1)?,
                start_date: r.get(2)?,
                end_date: r.get(3)?,
                from_text: r.get(4)?,
                to_text: r.get(5)?,
                program_name: r.get(6)?,
                position: r.get(7)?,
                duty: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 이 비교용 발급번호가 쓰인 적이 있는가 (상태와 상관없이).
pub fn issue_no_taken(conn: &Connection, issue_no_key: &str) -> AppResult<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM certificates WHERE issue_no_key = ?1", [issue_no_key], |_| Ok(()))
        .optional()?
        .is_some())
}

/// ISSUED → VOIDED. 그 밖의 칸은 건드리지 않는다(트리거가 지킨다).
pub fn void(conn: &Connection, id: i64, at: &str, reason: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE certificates SET status = 'VOIDED', voided_at = ?2, void_reason = ?3 WHERE id = ?1 AND status = 'ISSUED'",
        params![id, at, reason],
    )?;
    if n == 1 {
        Ok(())
    } else {
        Err(AppError::new("CERT_NOT_ISSUED", "이미 취소된 발급 건입니다."))
    }
}

// ---------------------------------------------------------------
// 출력 이력
// ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputRow {
    pub output_type: String,
    pub result: String,
    pub printer_name: Option<String>,
    pub copies: Option<i64>,
    pub detail: String,
    pub at: String,
}

pub fn add_output(conn: &Connection, certificate_id: i64, o: &OutputRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO certificate_outputs (certificate_id, output_type, result, printer_name, copies, detail, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![certificate_id, o.output_type, o.result, o.printer_name, o.copies, o.detail, o.at],
    )?;
    Ok(())
}

pub fn outputs(conn: &Connection, certificate_id: i64) -> AppResult<Vec<OutputRow>> {
    let mut stmt = conn.prepare(
        "SELECT output_type, result, printer_name, copies, detail, at FROM certificate_outputs
          WHERE certificate_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([certificate_id], |r| {
            Ok(OutputRow {
                output_type: r.get(0)?,
                result: r.get(1)?,
                printer_name: r.get(2)?,
                copies: r.get(3)?,
                detail: r.get(4)?,
                at: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
