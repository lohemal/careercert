//! 발급 확정 · 발급 기록 읽기 · 취소 · 출력 이력.
//!
//! ```text
//!   [내용 확인] prepare ──▶ review_token (개인정보 없는 문서 표현의 SHA-256)
//!   [발급 확정] issue   ──▶ 같은 요청으로 문서를 **다시 만들고** review_token 비교 → 경고 확인 → 발급번호 확인
//!                         → 주민번호·주소 봉인(AES-256-GCM) → 문서 지문(HMAC) → 스냅샷 INSERT  (한 트랜잭션)
//!   [정식 출력] for_output ─ 스냅샷 읽기 → 복호화 → CertificateDoc 재구성 → 지문 검증 → IssuedProof
//! ```
//!
//! 지키는 것
//!   * 정식 출력은 **DB 의 발급 기록에서만** 만든다. 작성 중 문서는 `IssuedProof` 를 만들 수 없다.
//!   * 지문이 맞지 않으면(스냅샷이 바뀌었으면) 출력하지 않는다.
//!   * 취소된 발급 건은 정식 출력하지 않는다.
//!   * 변경 기록에는 발급번호·성명·주민번호·주소·취소 사유 같은 값을 적지 않는다.

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto::{self, keys, Sealed};
use crate::domain::audit::Action;
use crate::domain::certificate::{self as cert, CertItem, CertificateDoc, Holder, RrnDisplay, SchoolBlock};
use crate::domain::{date, rrn};
use crate::error::{AppError, AppResult};
use crate::repo::{audit, certificate as repo};
use crate::service::certificate::{self as drafting, PrepareRequest};

/// 발급 기록에서 왔다는 증표. 정식 출력(`render::Mode::Issued`, `print::output::issued_*`)이 요구한다.
///
/// **이 모듈 밖에서는 만들 수 없다** — 만드는 함수 `from_issued_record` 가 이 모듈에만 보이고,
/// 필드 `_sealed` 도 밖에서 채울 수 없다. 그래서 증표는 `for_output`(발급 기록 읽기 → 복호화 →
/// 지문 검증 → 취소 여부 확인)을 지나야만 나온다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssuedProof {
    pub certificate_id: i64,
    _sealed: (),
}

impl IssuedProof {
    fn from_issued_record(certificate_id: i64) -> Self {
        IssuedProof { certificate_id, _sealed: () }
    }

    /// 시험 전용 — 양식 시험이 발급본 모양을 그려 보는 데 쓴다.
    #[cfg(test)]
    pub(crate) fn for_test(certificate_id: i64) -> Self {
        IssuedProof { certificate_id, _sealed: () }
    }
}

/// 지금 이 PC 에서 발급하는 사람 (서버로 옮기면 사용자 번호가 들어갈 자리)
pub const LOCAL_ACTOR: &str = "local";

pub const VOID_REASON_MAX: usize = 200;

/// 한 번에 인쇄할 수 있는 매수 (Phase 7 결정: 화면·서버 모두 1~5부)
pub const MAX_COPIES: u32 = 5;

pub fn check_copies(copies: u32) -> AppResult<()> {
    if (1..=MAX_COPIES).contains(&copies) {
        Ok(())
    } else {
        Err(AppError::invalid(format!("매수는 1~{MAX_COPIES}부 사이로 골라 주세요.")))
    }
}

/// 내용 확인 표 — 개인정보가 없는 문서 표현의 SHA-256.
pub fn review_token(doc: &CertificateDoc) -> String {
    crypto::plain_sha256(cert::canonical(doc, false).as_bytes())
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueRequest {
    /// 내용 확인 때와 같은 요청 (주민번호·주소 포함)
    pub prepare: PrepareRequest,
    /// 내용 확인 때 받은 표
    pub review_token: String,
    /// 담당자가 확인한 경고 (`Warning::ack_key`)
    pub acknowledged: Vec<String>,
    /// "이 내용으로 새 증명서 작성" 의 원본 발급 기록
    #[serde(default)]
    pub copied_from: Option<i64>,
}

impl std::fmt::Debug for IssueRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssueRequest")
            .field("prepare", &self.prepare)
            .field("acknowledged", &self.acknowledged)
            .finish_non_exhaustive()
    }
}

/// 암호문 안에 든 것. 직렬화한 평문은 다 쓰면 지운다.
#[derive(Serialize, Deserialize)]
struct Sensitive {
    rrn: String,
    address: String,
}

fn aad(uuid: &str, key_id: &str) -> Vec<u8> {
    format!("careercert/certificate/v1|{uuid}|{key_id}").into_bytes()
}

/// 발급 확정. `Db::write` 안(한 트랜잭션)에서 부른다. 새 발급 기록 번호를 돌려준다.
pub fn issue(conn: &Connection, req: IssueRequest, now: &str, actor: &str) -> AppResult<i64> {
    // 1) 지금 자료로 문서를 다시 만든다 — 검사는 내용 확인과 같은 길(domain::certificate::build)
    let built = drafting::prepare(conn, req.prepare.clone(), now)?;
    let doc = built.doc;

    // 2) 내용 확인 뒤에 강사·경력·학교 설정이 바뀌었으면 발급하지 않는다
    if review_token(&doc) != req.review_token {
        return Err(AppError::new(
            "CERT_CHANGED",
            "내용 확인 뒤에 자료가 바뀌었습니다. 발급하지 않았습니다. [증명서 내용 확인] 을 다시 해 주세요.",
        ));
    }

    // 3) 확정이 필요한 경고는 하나하나 확인했어야 한다
    let missing: Vec<String> = built
        .warnings
        .iter()
        .filter(|w| !req.acknowledged.contains(&w.ack_key()))
        .map(|w| w.message.clone())
        .collect();
    if !missing.is_empty() {
        return Err(AppError::new(
            "CERT_WARNINGS_UNCONFIRMED",
            format!("확인하지 않은 경고가 {}건 있습니다. 경고를 모두 확인해야 발급할 수 있습니다.", missing.len()),
        ));
    }

    // 4) 발급번호는 상태와 상관없이 한 번만 쓴다 (DB UNIQUE 도 막는다)
    let key_no = cert::issue_no_key(&doc.issue_no);
    if repo::issue_no_taken(conn, &key_no)? {
        return Err(AppError::new(
            "CERT_ISSUE_NO_TAKEN",
            "이미 사용한 발급번호입니다(취소된 발급 포함). NEIS 에서 받은 새 번호를 입력해 주세요.",
        ));
    }

    // 4-1) "이 내용으로 새 증명서 작성" 의 원본은 있는 발급 기록이어야 한다 (내용은 읽지 않는다)
    if let Some(from) = req.copied_from {
        if repo::find_meta(conn, from)?.is_none() {
            return Err(AppError::not_found("참고한 원래 발급 기록을 찾을 수 없습니다."));
        }
    }

    // 5) 봉인 — 키가 없으면 이때 처음 만든다
    let key = keys::current_or_create(conn, now)?;
    let uuid = uuid::Uuid::new_v4().to_string();
    let plain = Zeroizing::new(serde_json::to_vec(&Sensitive {
        rrn: doc.holder.rrn.as_str().to_string(),
        address: doc.holder.address.clone(),
    })?);
    let sealed = crypto::seal(&key, &aad(&uuid, &key.key_id), &plain)?;
    drop(plain);
    let doc_hash = crypto::doc_hash(&key, cert::canonical(&doc, true).as_bytes());

    // 6) 스냅샷
    let issued_on = date::to_iso(doc.issued_on);
    let masked = doc.holder.rrn.masked();
    let id = repo::insert(
        conn,
        &repo::NewCertificate {
            uuid: &uuid,
            issue_no: &doc.issue_no,
            issue_no_key: &key_no,
            issued_on: &issued_on,
            template_version: doc.template_version,
            title: &doc.title,
            purpose: &doc.purpose,
            holder_name: &doc.holder.name,
            sensitive_nonce: &sealed.nonce,
            sensitive_cipher: &sealed.ciphertext,
            key_id: &key.key_id,
            masked_rrn: &masked,
            rrn_display: doc.rrn_display.code(),
            issuer_title: &doc.school.issuer_title,
            department: &doc.school.department,
            manager_name: &doc.school.manager_name,
            phone: &doc.school.phone,
            item_count: doc.items.len(),
            doc_hash: &doc_hash,
            source_instructor_id: Some(req.prepare.instructor_id),
            copied_from_certificate_id: req.copied_from,
            created_at: now,
            created_by: actor,
        },
    )?;
    for (i, it) in doc.items.iter().enumerate() {
        repo::insert_item(
            conn,
            id,
            &repo::ItemRow {
                seq: i as i64 + 1,
                source_career_id: Some(it.source_career_id),
                start_date: date::to_iso(it.start_date),
                end_date: it.end_date.map(date::to_iso),
                from_text: it.from_text.clone(),
                to_text: it.to_text.clone(),
                program_name: it.program_name.clone(),
                position: it.position.clone(),
                duty: it.duty.clone(),
            },
        )?;
    }
    audit::add(
        conn,
        now,
        Action::CertificateIssue,
        Some(id),
        &format!("발급 · 경력 {}건 · 주민번호 표시 {}", doc.items.len(), doc.rrn_display.code()),
    )?;
    Ok(id)
}

/// 발급 기록에서 다시 만든 문서와 그 기록. (Debug 는 주민번호·주소를 가린다 — Holder·Row 의 Debug)
#[derive(Debug)]
pub struct Reconstructed {
    pub row: repo::Row,
    pub doc: CertificateDoc,
}

fn bad_snapshot(what: &str) -> AppError {
    AppError::new(
        "CERT_TAMPERED",
        "발급 기록이 손상되었거나 바뀌었습니다. 이 증명서는 출력할 수 없습니다.",
    )
    .detail(what.to_string())
}

fn day(s: &str) -> AppResult<NaiveDate> {
    date::parse_iso(s).map_err(|_| bad_snapshot("date"))
}

/// 스냅샷을 읽고, 주민번호·주소를 풀고, 문서를 다시 만들고, **지문을 검증**한다.
/// 지금 강사·경력·학교 설정은 읽지 않는다.
pub fn reconstruct(conn: &Connection, id: i64) -> AppResult<Reconstructed> {
    reconstruct_with(conn, id, &mut KeyCache::default())
}

/// 여러 발급본을 이어서 검증할 때 쓰는 키 보관 — 같은 키 번호를 매번 DPAPI 로 다시 풀지 않는다.
/// (검증 단계는 그대로다: 발급본마다 복호화·문서 재구성·지문 비교를 모두 한다.)
#[derive(Default)]
pub struct KeyCache(std::collections::HashMap<String, crypto::DataKey>);

/// `reconstruct` 와 같다. 키만 `cache` 에서 꺼내 쓴다.
pub fn reconstruct_with(conn: &Connection, id: i64, cache: &mut KeyCache) -> AppResult<Reconstructed> {
    let row = repo::get(conn, id)?;
    let items = repo::items(conn, id)?;
    if items.len() as i64 != row.item_count {
        return Err(bad_snapshot("item count"));
    }
    if !cache.0.contains_key(&row.key_id) {
        let k = keys::get(conn, &row.key_id)?;
        cache.0.insert(row.key_id.clone(), k);
    }
    let key = &cache.0[&row.key_id];
    let plain = crypto::open(
        key,
        &aad(&row.uuid, &row.key_id),
        &Sealed { nonce: row.sensitive_nonce.clone(), ciphertext: row.sensitive_cipher.clone() },
    )?;
    let sensitive: Sensitive = serde_json::from_slice(&plain).map_err(|_| bad_snapshot("payload"))?;
    drop(plain);
    let holder_rrn = rrn::parse(&sensitive.rrn).map_err(|_| bad_snapshot("rrn"))?;
    let rrn_display = match row.rrn_display.as_str() {
        "FULL" => RrnDisplay::Full,
        "MASK_BACK" => RrnDisplay::MaskBack,
        _ => return Err(bad_snapshot("rrn_display")),
    };
    let doc = CertificateDoc {
        template_version: row.template_version,
        title: row.title.clone(),
        issue_no: row.issue_no.clone(),
        issued_on: day(&row.issued_on)?,
        purpose: row.purpose.clone(),
        holder: Holder { name: row.holder_name.clone(), rrn: holder_rrn, address: sensitive.address },
        rrn_display,
        items: items
            .iter()
            .map(|it| {
                Ok(CertItem {
                    source_career_id: it.source_career_id.unwrap_or(0),
                    start_date: day(&it.start_date)?,
                    end_date: it.end_date.as_deref().map(day).transpose()?,
                    from_text: it.from_text.clone(),
                    to_text: it.to_text.clone(),
                    program_name: it.program_name.clone(),
                    position: it.position.clone(),
                    duty: it.duty.clone(),
                })
            })
            .collect::<AppResult<Vec<_>>>()?,
        school: SchoolBlock {
            issuer_title: row.issuer_title.clone(),
            department: row.department.clone(),
            manager_name: row.manager_name.clone(),
            phone: row.phone.clone(),
        },
    };
    if crypto::doc_hash(key, cert::canonical(&doc, true).as_bytes()) != row.doc_hash {
        return Err(bad_snapshot("doc_hash"));
    }
    Ok(Reconstructed { row, doc })
}

/// 정식 출력에 쓸 문서와 증표. 취소된 발급 건은 거절한다.
pub fn for_output(conn: &Connection, id: i64) -> AppResult<(CertificateDoc, IssuedProof)> {
    let r = reconstruct(conn, id)?;
    if r.row.status != "ISSUED" {
        return Err(AppError::new("CERT_VOIDED", "취소된 발급 건입니다. 정식 출력할 수 없습니다."));
    }
    Ok((r.doc, IssuedProof::from_issued_record(r.row.id)))
}

/// 발급 취소. 사유는 필수. 스냅샷 내용은 바꾸지 않는다.
pub fn void(conn: &Connection, id: i64, reason: &str, now: &str) -> AppResult<()> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(AppError::invalid("취소 사유를 입력해 주세요."));
    }
    if reason.chars().count() > VOID_REASON_MAX {
        return Err(AppError::invalid(format!("취소 사유는 {VOID_REASON_MAX}자 이내로 입력해 주세요.")));
    }
    let row = repo::get(conn, id)?;
    if row.status != "ISSUED" {
        return Err(AppError::new("CERT_NOT_ISSUED", "이미 취소된 발급 건입니다."));
    }
    repo::void(conn, id, now, reason)?;
    // 사유는 기록에 옮겨 적지 않는다 — 개인정보가 들어 있을 수 있다(발급 기록에만 남는다)
    audit::add(conn, now, Action::CertificateVoid, Some(id), "발급 취소")?;
    Ok(())
}

/// 출력 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    Pdf,
    Print { copies: u32 },
}

/// 정식 출력 결과를 남긴다. **성공했을 때만 SUCCESS.** 실패는 FAILED 로 따로.
pub fn record_output(
    conn: &Connection,
    id: i64,
    kind: OutputKind,
    printer: Option<&str>,
    success: bool,
    detail: &str,
    now: &str,
) -> AppResult<()> {
    let (output_type, printer_name, copies) = match kind {
        OutputKind::Pdf => ("PDF", None, None),
        OutputKind::Print { copies } => {
            check_copies(copies)?;
            (
            "PRINT",
            Some(printer.unwrap_or("(기본 프린터)").to_string()),
            Some(copies as i64),
        )
        }
    };
    repo::add_output(
        conn,
        id,
        &repo::OutputRow {
            output_type: output_type.into(),
            result: if success { "SUCCESS" } else { "FAILED" }.into(),
            printer_name,
            copies,
            detail: detail.chars().take(200).collect(),
            at: now.into(),
        },
    )
}

#[cfg(test)]
#[path = "issuance_tests.rs"]
pub(crate) mod issuance_tests;
