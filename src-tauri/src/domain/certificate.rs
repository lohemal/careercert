//! 증명서 내용 — **발급일을 기준으로** 만든다.
//!
//! `build` 가 증명서 내용을 만드는 **유일한 길**이다. 작성 화면의 내용 확인, Phase 5 출력,
//! Phase 6 발급 확정이 모두 이 함수를 지난다.
//!
//! 발급일 기준 기간 (Phase 4 결정 — 예정 종료일은 보지 않는다)
//!
//! ```text
//!   시작일 > 발급일                  → 넣을 수 없다 (아직 시작하지 않은 경력)
//!   재직중                           → 시작일 ~ 현재
//!   종료, 발급일 ≥ 종료일            → 시작일 ~ 종료일
//!   종료, 발급일 < 종료일            → 시작일 ~ 현재   (발급일에는 아직 근무 중이었다)
//! ```
//!
//! 지금 DB 상태를 그대로 찍지 않는다. 9월 30일에 발급하는 증명서는 9월 30일의 사실이어야 한다.
//!
//! 개인정보: 주민번호·주소는 이 구조체 안에만 있다. `Debug` 는 값을 찍지 않는다.

use std::collections::BTreeSet;

use chrono::NaiveDate;
use serde::Deserialize;

use super::career::{Career, CareerFields, Term};
use super::date;
use super::instructor::Instructor;
use super::rrn::{self, Rrn};
use super::settings::{missing, SchoolSettings};
use crate::error::{AppError, AppResult};

/// 증명서 양식 판. 양식이 바뀌면 올리고, 옛 판은 지우지 않는다(Phase 5·6).
/// * 1판 — v0.1.0·v0.1.1 발급본. **동결**(render::v1, 정규 표현에 로고 줄 없음).
/// * 2판 — v0.1.2 부터 새로 발급. `경력 사항` 가운데 정렬 · 학교 로고 워터마크(정규 표현에 `logo=` 줄).
pub const TEMPLATE_VERSION: u32 = 2;

pub const ISSUE_NO_MAX: usize = 50;
pub const PURPOSE_MAX: usize = 50;
pub const ADDRESS_MAX: usize = 200;

// ---------------------------------------------------------------
// 발급일 기준 기간
// ---------------------------------------------------------------

/// 증명서에 넣을 수 없는 까닭.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ineligible {
    /// 보관한 경력
    Archived,
    /// 발급일에 아직 시작하지 않았다
    NotStarted,
}

impl Ineligible {
    pub fn message(self, issued_on: NaiveDate) -> String {
        match self {
            Ineligible::Archived => "보관된 경력은 증명서에 넣을 수 없습니다.".into(),
            Ineligible::NotStarted => format!(
                "발급일({})에는 아직 시작하지 않은 경력이라 넣을 수 없습니다.",
                date::display(issued_on)
            ),
        }
    }
}

/// 발급일에 본 끝 — `None` 이면 `현재`.
///
/// 예정 종료일(`planned_end_date`)은 **보지 않는다.**
pub fn end_on(f: &CareerFields, issued_on: NaiveDate) -> Result<Option<NaiveDate>, Ineligible> {
    if f.start_date > issued_on {
        return Err(Ineligible::NotStarted);
    }
    Ok(match f.term {
        Term::Active => None,
        Term::Ended { end_date, .. } if issued_on >= end_date => Some(end_date),
        Term::Ended { .. } => None,
    })
}

/// 이 경력을 이 발급일의 증명서에 넣을 수 있는가. 된다면 발급일 기준 끝.
pub fn eligibility(c: &Career, issued_on: NaiveDate) -> Result<Option<NaiveDate>, Ineligible> {
    if c.is_archived() {
        return Err(Ineligible::Archived);
    }
    end_on(&c.fields, issued_on)
}

// ---------------------------------------------------------------
// 증명서 내용
// ---------------------------------------------------------------

/// 화면에서 받은 발급 정보. 주민번호·주소가 들어 있으므로 `Debug` 가 값을 찍지 않는다.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueInput {
    pub rrn: String,
    pub address: String,
    /// NEIS 발급번호 — 앞뒤 공백만 정리하고 **그대로** 쓴다(결정 D9)
    pub issue_no: String,
    pub purpose: String,
    /// `YYYY-MM-DD`
    pub issued_on: String,
    /// 증명서에 주민번호 뒷자리를 가려 찍는가 (기본 꺼짐 = 전체 표시)
    #[serde(default)]
    pub mask_rrn: bool,
}

impl std::fmt::Debug for IssueInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssueInput")
            .field("rrn", &"******")
            .field("address", &"******")
            .field("issue_no", &self.issue_no)
            .field("purpose", &self.purpose)
            .field("issued_on", &self.issued_on)
            .field("mask_rrn", &self.mask_rrn)
            .finish()
    }
}

/// 증명서에 주민번호를 어떻게 찍는가. 원본(`Holder::rrn`)은 언제나 전체다 — 이 값은 **표시 방식**이다.
/// 발급 기록(Phase 6)에 함께 저장해, 그 증명서가 어떤 방식으로 나갔는지 남긴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RrnDisplay {
    /// `YYMMDD-NNNNNNN` (전체)
    Full,
    /// `YYMMDD-N******` (뒷자리 가림)
    MaskBack,
}

impl RrnDisplay {
    pub fn code(self) -> &'static str {
        match self {
            RrnDisplay::Full => "FULL",
            RrnDisplay::MaskBack => "MASK_BACK",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Holder {
    pub name: String,
    pub rrn: Rrn,
    pub address: String,
}

impl std::fmt::Debug for Holder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Holder")
            .field("name", &self.name)
            .field("rrn", &self.rrn)
            .field("address", &"******")
            .finish()
    }
}

/// 증명서의 경력 한 줄.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertItem {
    /// 어느 경력에서 왔는가 — 추적용. **출력에는 쓰지 않는다.**
    pub source_career_id: i64,
    pub start_date: NaiveDate,
    /// 발급일 기준 끝. None = `현재`
    pub end_date: Option<NaiveDate>,
    /// 증명서 '부터' 칸 `2026.03.04`
    pub from_text: String,
    /// 증명서 '까지' 칸 `현재` / `2026.12.31`
    pub to_text: String,
    pub program_name: String,
    pub position: String,
    pub duty: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchoolBlock {
    pub issuer_title: String,
    pub department: String,
    pub manager_name: String,
    pub phone: String,
}

/// 증명서 한 장의 내용 전부. 이것만 있으면 증명서를 똑같이 다시 그릴 수 있어야 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateDoc {
    pub template_version: u32,
    pub title: String,
    pub issue_no: String,
    pub issued_on: NaiveDate,
    pub purpose: String,
    pub holder: Holder,
    /// 주민번호 표시 방식 (원본은 holder.rrn 그대로)
    pub rrn_display: RrnDisplay,
    pub items: Vec<CertItem>,
    pub school: SchoolBlock,
    /// 발급 당시 학교 로고 (2판부터, 없을 수 있다). 1판은 언제나 None.
    pub logo: Option<super::logo::Logo>,
}

/// 막지는 않지만 담당자가 알아야 할 것.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// ISSUE_FAR_PAST · ENDS_AFTER_ISSUE · RRN_BIRTH_DATE
    pub code: &'static str,
    pub message: String,
    /// 특정 경력에 걸린 경고면 그 번호
    pub career_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub doc: CertificateDoc,
    pub warnings: Vec<Warning>,
}

fn err(code: &str, msg: impl Into<String>) -> AppError {
    AppError::new(code, msg)
}

/// 증명서 내용을 만든다. **아무것도 저장하지 않는다.**
///
/// * `careers` — 담당자가 고른 경력(순서 무관, 같은 번호 여러 번 무관). 모두 이 강사의 것이고,
///   보관되지 않았고, 발급일에 이미 시작했어야 한다. 하나라도 아니면 **만들지 않는다.**
/// * `school` — 지금 학교 설정. 증명서에 찍히는 칸이 비어 있으면 만들지 않는다.
/// * `today` — 발급일이 오늘보다 뒤면 **거부**하고(Phase 5 결정), 1년 넘게 과거면 경고한다.
pub fn build(
    instructor: &Instructor,
    careers: &[Career],
    input: IssueInput,
    school: &SchoolSettings,
    today: NaiveDate,
) -> AppResult<Built> {
    if instructor.is_archived() {
        return Err(err("CERT_INSTRUCTOR_ARCHIVED", "보관된 강사의 증명서는 만들 수 없습니다. 먼저 보관을 해제해 주세요."));
    }

    // ---- 학교 설정 ----
    let gaps = missing(school);
    if !gaps.is_empty() {
        let labels: Vec<&str> = gaps.iter().map(|k| k.label()).collect();
        return Err(err(
            "CERT_SCHOOL_INCOMPLETE",
            format!("학교 기본정보가 비어 있습니다: {}. 설정에서 채워 주세요.", labels.join(", ")),
        ));
    }

    // ---- 발급 정보 ----
    let issued_on = date::parse_iso(&input.issued_on)
        .map_err(|e| err(e.code(), format!("발급일: {}", e.message())))?;
    if issued_on > today {
        return Err(err(
            "CERT_ISSUE_IN_FUTURE",
            format!(
                "발급일({})이 오늘({})보다 뒤입니다. 미래 날짜로는 발급할 수 없습니다.",
                date::display(issued_on),
                date::display(today)
            ),
        ));
    }
    let issue_no = input.issue_no.trim().to_string(); // 앞뒤 공백 말고는 손대지 않는다 (D9)
    if issue_no.is_empty() {
        return Err(err("CERT_ISSUE_NO_REQUIRED", "발급번호를 입력해 주세요. NEIS 민원 등록 후 받은 번호를 그대로 적습니다."));
    }
    if issue_no.chars().count() > ISSUE_NO_MAX {
        return Err(err("CERT_ISSUE_NO_TOO_LONG", format!("발급번호는 {ISSUE_NO_MAX}자 이내입니다.")));
    }
    let purpose = input.purpose.trim().to_string();
    if purpose.is_empty() {
        return Err(err("CERT_PURPOSE_REQUIRED", "용도를 입력해 주세요."));
    }
    if purpose.chars().count() > PURPOSE_MAX {
        return Err(err("CERT_PURPOSE_TOO_LONG", format!("용도는 {PURPOSE_MAX}자 이내입니다.")));
    }
    let rrn = rrn::parse(&input.rrn)?;
    let address = input.address.trim().to_string();
    if address.is_empty() {
        return Err(err("CERT_ADDRESS_REQUIRED", "주소를 입력해 주세요."));
    }
    if address.chars().count() > ADDRESS_MAX {
        return Err(err("CERT_ADDRESS_TOO_LONG", format!("주소는 {ADDRESS_MAX}자 이내입니다.")));
    }

    // ---- 경력 ----
    let mut seen = BTreeSet::new();
    let mut picked: Vec<&Career> = careers.iter().filter(|c| seen.insert(c.id)).collect();
    if picked.is_empty() {
        return Err(err("CERT_NO_CAREERS", "증명서에 넣을 경력을 하나 이상 골라 주세요."));
    }
    picked.sort_by_key(|c| (c.fields.start_date, c.id));

    let mut items = Vec::with_capacity(picked.len());
    let mut warnings = Vec::new();
    for c in picked {
        let label = format!("{} · {}", c.fields.program_name, c.period());
        if c.instructor_id != instructor.id {
            return Err(err("CERT_CAREER_OTHER", format!("다른 강사의 경력이 섞여 있습니다({label}).")));
        }
        let end = eligibility(c, issued_on).map_err(|why| {
            let code = match why {
                Ineligible::Archived => "CERT_CAREER_ARCHIVED",
                Ineligible::NotStarted => "CERT_CAREER_NOT_STARTED",
            };
            err(code, format!("{} ({label})", why.message(issued_on)))
        })?;
        // 실제로는 끝났지만 발급일에는 아직 근무 중이었다 → '현재' 로 찍힌다는 것을 알린다
        if let (None, Some(real_end)) = (end, c.fields.term.end_date()) {
            warnings.push(Warning {
                code: "ENDS_AFTER_ISSUE",
                message: format!(
                    "{label}: 종료일({})이 발급일 뒤라 증명서에는 '현재'로 표시됩니다.",
                    date::display(real_end)
                ),
                career_id: Some(c.id),
            });
        }
        let (from_text, to_text) = date::display_period_parts(c.fields.start_date, end);
        items.push(CertItem {
            source_career_id: c.id,
            start_date: c.fields.start_date,
            end_date: end,
            from_text,
            to_text,
            program_name: c.fields.program_name.clone(),
            position: c.fields.position.clone(),
            duty: c.fields.duty.clone(),
        });
    }

    // ---- 경고 ----
    if (today - issued_on).num_days() > 365 {
        warnings.insert(0, Warning {
            code: "ISSUE_FAR_PAST",
            message: format!("발급일({})이 1년도 더 지난 날입니다.", date::display(issued_on)),
            career_id: None,
        });
    }
    if !rrn.birth_date_ok() {
        warnings.push(Warning {
            code: "RRN_BIRTH_DATE",
            message: "주민등록번호 앞자리의 생년월일이 달력에 없는 날입니다. 잘못 입력하지 않았는지 확인하세요.".into(),
            career_id: None,
        });
    }

    Ok(Built {
        doc: CertificateDoc {
            template_version: TEMPLATE_VERSION,
            title: school.certificate_title.clone(),
            issue_no,
            issued_on,
            purpose,
            holder: Holder {
                name: instructor.name.clone(),
                rrn,
                address,
            },
            rrn_display: if input.mask_rrn { RrnDisplay::MaskBack } else { RrnDisplay::Full },
            items,
            school: SchoolBlock {
                issuer_title: school.issuer_title.clone(),
                department: school.department.clone(),
                manager_name: school.manager_name.clone(),
                phone: school.phone.clone(),
            },
            // 로고는 service 가 학교 설정에서 붙인다(이 함수는 DB 를 모른다)
            logo: None,
        },
        warnings,
    })
}

impl Warning {
    /// 확정 창의 확인 체크 하나를 가리키는 이름 — `ENDS_AFTER_ISSUE:12`, `ISSUE_FAR_PAST`.
    pub fn ack_key(&self) -> String {
        match self.career_id {
            Some(id) => format!("{}:{id}", self.code),
            None => self.code.to_string(),
        }
    }
}

// ---------------------------------------------------------------
// 정규화한 문서 표현 (문서 지문의 입력)
// ---------------------------------------------------------------

/// 문서 지문용 정규 표현 판 — 바꾸면 옛 발급 기록의 지문이 맞지 않으므로 **고치지 않는다.**
pub const CANONICAL_VERSION: &str = "CERTDOC/1";

/// 정규 표현. 한 줄에 `이름=값` 하나, **줄 차례가 고정**이다(구조체 필드 차례나 JSON 직렬화에 기대지 않는다).
/// 값의 `\` 는 `\\`, 줄바꿈은 `\n`·`\r`(두 글자)로 바꿔 한 줄을 지킨다. 경력 줄은 seq 차례.
///
/// ```text
/// CERTDOC/1
/// template_version=1
/// title=… / issue_no=… / issued_on=YYYY-MM-DD / purpose=…
/// holder.name=… / holder.rrn=… / holder.address=…        ← include_pii=false 이면 이 두 줄 대신 '-'
/// rrn_display=FULL|MASK_BACK
/// school.issuer_title=… / school.department=… / school.manager_name=… / school.phone=…
/// items=N
/// item.1.source=<career id> / item.1.start=… / item.1.end=YYYY-MM-DD|NONE / item.1.from=… / item.1.to=…
/// item.1.program=… / item.1.position=… / item.1.duty=…
/// ```
///
/// * `include_pii = true`  — 발급 기록의 지문(HMAC)용. **결과를 로그·파일에 남기지 않는다.**
/// * `include_pii = false` — 내용 확인과 발급 확정 사이에 자료가 바뀌었는지 보는 표(그냥 SHA-256).
pub fn canonical(doc: &CertificateDoc, include_pii: bool) -> zeroize::Zeroizing<String> {
    fn e(v: &str) -> String {
        v.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r")
    }
    let mut out = String::with_capacity(1024);
    out.push_str(CANONICAL_VERSION);
    out.push('\n');
    let mut line = |k: &str, v: &str| {
        out.push_str(k);
        out.push('=');
        out.push_str(&e(v));
        out.push('\n');
    };
    line("template_version", &doc.template_version.to_string());
    line("title", &doc.title);
    line("issue_no", &doc.issue_no);
    line("issued_on", &date::to_iso(doc.issued_on));
    line("purpose", &doc.purpose);
    line("holder.name", &doc.holder.name);
    if include_pii {
        line("holder.rrn", doc.holder.rrn.as_str());
        line("holder.address", &doc.holder.address);
    } else {
        line("holder.rrn", "-");
        line("holder.address", "-");
    }
    line("rrn_display", doc.rrn_display.code());
    line("school.issuer_title", &doc.school.issuer_title);
    line("school.department", &doc.school.department);
    line("school.manager_name", &doc.school.manager_name);
    line("school.phone", &doc.school.phone);
    // 2판부터: 로고 보관본의 지문(없으면 NONE). 1판에는 이 줄이 없다 — 옛 발급본의 doc_hash 가 그대로다.
    if doc.template_version >= 2 {
        line("logo", doc.logo.as_ref().map(|l| l.sha256.as_str()).unwrap_or("NONE"));
    }
    line("items", &doc.items.len().to_string());
    for (i, it) in doc.items.iter().enumerate() {
        let n = i + 1;
        line(&format!("item.{n}.source"), &it.source_career_id.to_string());
        line(&format!("item.{n}.start"), &date::to_iso(it.start_date));
        line(&format!("item.{n}.end"), &it.end_date.map(date::to_iso).unwrap_or_else(|| "NONE".into()));
        line(&format!("item.{n}.from"), &it.from_text);
        line(&format!("item.{n}.to"), &it.to_text);
        line(&format!("item.{n}.program"), &it.program_name);
        line(&format!("item.{n}.position"), &it.position);
        line(&format!("item.{n}.duty"), &it.duty);
    }
    zeroize::Zeroizing::new(out)
}

/// 공백을 모두 뺀 발급번호 — 중복 비교용(출력에는 원문을 쓴다).
pub fn issue_no_key(issue_no: &str) -> String {
    issue_no.chars().filter(|c| !c.is_whitespace()).collect()
}

/// 정식 PDF 의 기본 파일 이름 `경력증명서_김으뜸_제2026-152호.pdf`.
///
/// * 성명과 발급번호**만** 쓴다 — 주민번호·주소는 절대 넣지 않는다.
/// * Windows 가 받지 않는 글자(`\ / : * ? " < > |`, 제어 문자)는 `_` 로 바꾼다. **파일 이름에서만**
///   바꾸는 것이고 발급번호 값 자체는 그대로다(D9).
/// * 끝의 점·공백을 떼고, 장치 이름(CON·PRN·AUX·NUL·COM1… LPT1…)은 피하고, 너무 길면 자른다.
pub fn pdf_file_name(doc: &CertificateDoc) -> String {
    let stem = format!("경력증명서_{}_{}", safe_part(&doc.holder.name), safe_part(&doc.issue_no));
    let stem: String = stem.chars().take(120).collect();
    let stem = stem.trim_end_matches(['.', ' ']).to_string();
    format!("{stem}.pdf")
}

fn safe_part(s: &str) -> String {
    let cleaned: String = s
        .trim()
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim_matches(['.', ' ']).to_string();
    let upper = cleaned.to_ascii_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit());
    if cleaned.is_empty() {
        "_".into()
    } else if reserved {
        format!("_{cleaned}")
    } else {
        cleaned
    }
}

#[cfg(test)]
#[path = "certificate_tests.rs"]
mod certificate_tests;
