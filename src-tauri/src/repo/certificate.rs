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
    /// 2판부터 — 발급 당시 로고 보관본의 지문
    pub logo_sha256: Option<&'a str>,
}

/// 새 발급 기록의 번호. 오발급 폐기로 지운 번호를 다시 쓰지 않는다 — 변경 기록(audit_log)이 발급 번호로
/// 가리키므로, 지금 있는 발급 기록과 변경 기록에 나온 발급 번호 중 가장 큰 것 + 1.
const NEXT_ID: &str = "(SELECT MAX(m) + 1 FROM (
        SELECT COALESCE(MAX(id), 0) AS m FROM certificates
        UNION ALL
        SELECT COALESCE(MAX(target_id), 0) FROM audit_log WHERE target_type = 'certificate'))";

pub fn insert(conn: &Connection, c: &NewCertificate<'_>) -> AppResult<i64> {
    conn.execute(
        &format!("INSERT INTO certificates (id, uuid, issue_no, issue_no_key, issued_on, template_version, title, purpose,
                                   holder_name, sensitive_nonce, sensitive_cipher, key_id, masked_rrn, rrn_display,
                                   issuer_title, department, manager_name, phone, item_count, doc_hash,
                                   source_instructor_id, copied_from_certificate_id, created_at, created_by, logo_sha256)
         VALUES ({NEXT_ID}, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)"),
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
            c.logo_sha256,
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
    pub logo_sha256: Option<String>,
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
                    source_instructor_id, copied_from_certificate_id, created_at, created_by, logo_sha256";

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
        logo_sha256: r.get(26)?,
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

// ---------------------------------------------------------------
// 발급이력 — 목록·상세용 (암호문은 읽지 않는다)
// ---------------------------------------------------------------

/// 발급 기록에서 **암호문이 아닌 칸만**. 목록·상세·대시보드·"새 증명서 작성" 은 이것만 읽는다 —
/// `sensitive_*`·`key_id`·`doc_hash` 를 SELECT 하지 않으므로 복호화할 거리가 아예 없다.
#[derive(Clone, PartialEq, Eq)]
pub struct Meta {
    pub id: i64,
    pub issue_no: String,
    pub issued_on: String,
    pub title: String,
    pub purpose: String,
    pub holder_name: String,
    pub masked_rrn: String,
    pub rrn_display: String,
    pub issuer_title: String,
    pub department: String,
    pub manager_name: String,
    pub phone: String,
    pub item_count: i64,
    pub status: String,
    pub voided_at: Option<String>,
    pub void_reason: Option<String>,
    pub source_instructor_id: Option<i64>,
    pub copied_from_certificate_id: Option<i64>,
    pub created_at: String,
    /// 발급 당시 로고가 있었는가 (2판부터)
    pub has_logo: bool,
    pub template_version: u32,
}

impl std::fmt::Debug for Meta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("certificates::Meta")
            .field("id", &self.id)
            .field("issue_no", &self.issue_no)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

const META_COLS: &str = "id, issue_no, issued_on, title, purpose, holder_name, masked_rrn, rrn_display, issuer_title,
                         department, manager_name, phone, item_count, status, voided_at, void_reason,
                         source_instructor_id, copied_from_certificate_id, created_at, logo_sha256 IS NOT NULL, template_version";

fn meta_from_row(r: &SqlRow<'_>) -> rusqlite::Result<Meta> {
    Ok(Meta {
        id: r.get(0)?,
        issue_no: r.get(1)?,
        issued_on: r.get(2)?,
        title: r.get(3)?,
        purpose: r.get(4)?,
        holder_name: r.get(5)?,
        masked_rrn: r.get(6)?,
        rrn_display: r.get(7)?,
        issuer_title: r.get(8)?,
        department: r.get(9)?,
        manager_name: r.get(10)?,
        phone: r.get(11)?,
        item_count: r.get(12)?,
        status: r.get(13)?,
        voided_at: r.get(14)?,
        void_reason: r.get(15)?,
        source_instructor_id: r.get(16)?,
        copied_from_certificate_id: r.get(17)?,
        created_at: r.get(18)?,
        has_logo: r.get(19)?,
        template_version: r.get(20)?,
    })
}

pub fn find_meta(conn: &Connection, id: i64) -> AppResult<Option<Meta>> {
    Ok(conn
        .query_row(&format!("SELECT {META_COLS} FROM certificates WHERE id = ?1"), [id], meta_from_row)
        .optional()?)
}

pub fn get_meta(conn: &Connection, id: i64) -> AppResult<Meta> {
    find_meta(conn, id)?.ok_or_else(|| AppError::not_found("발급 기록을 찾을 수 없습니다."))
}

/// 목록 조건 (service 가 검사한 값만).
pub struct ListFilter<'a> {
    /// 성명·발급번호 일부 (앞뒤 공백 정리됨, 빈 글자 = 조건 없음)
    pub query: &'a str,
    /// 발급일 이후 (YYYY-MM-DD, 그날 포함)
    pub from: Option<&'a str>,
    /// 발급일 이전 (그날 포함)
    pub to: Option<&'a str>,
    /// ISSUED · VOIDED · None(전체)
    pub status: Option<&'a str>,
    pub limit: i64,
}

/// 발급이력 목록 — 최근 발급순(발급일 내림차순, 같은 날은 나중에 확정한 것 먼저).
///
/// 기존 강사 찾기와 같이 `LIKE` 대신 `instr` — 검색어의 `%`·`_` 가 와일드카드가 되지 않는다.
/// 발급번호는 공백을 뺀 비교용 번호(`issue_no_key`)로도 찾는다(`2026-가- 0001` ↔ `2026-가-0001`).
/// 주민번호·주소로는 찾지 않는다(암호문이라 찾을 수도 없다).
pub fn search(conn: &Connection, f: &ListFilter<'_>) -> AppResult<Vec<Meta>> {
    let key: String = f.query.chars().filter(|c| !c.is_whitespace()).collect();
    let mut stmt = conn.prepare(&format!(
        "SELECT {META_COLS} FROM certificates
          WHERE (?1 = '' OR instr(holder_name, ?1) > 0 OR instr(issue_no, ?1) > 0
                 OR (?2 <> '' AND instr(issue_no_key, ?2) > 0))
            AND (?3 IS NULL OR issued_on >= ?3)
            AND (?4 IS NULL OR issued_on <= ?4)
            AND (?5 IS NULL OR status = ?5)
          ORDER BY issued_on DESC, id DESC
          LIMIT ?6"
    ))?;
    let rows = stmt
        .query_map(params![f.query, key, f.from, f.to, f.status, f.limit], meta_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 최근 확정한 발급 건 (발급 상태만).
pub fn recent_issued(conn: &Connection, limit: i64) -> AppResult<Vec<Meta>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {META_COLS} FROM certificates WHERE status = 'ISSUED' ORDER BY id DESC LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit], meta_from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 최근 취소한 발급 건 (취소 시각 차례).
pub fn recent_voided(conn: &Connection, limit: i64) -> AppResult<Vec<Meta>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {META_COLS} FROM certificates WHERE status = 'VOIDED' ORDER BY voided_at DESC, id DESC LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit], meta_from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 상태별 건수 (발급, 취소)
pub fn counts(conn: &Connection) -> AppResult<(i64, i64)> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(status = 'ISSUED'), 0), COALESCE(SUM(status = 'VOIDED'), 0) FROM certificates",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?)
}
