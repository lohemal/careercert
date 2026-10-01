//! 발급이력 — 찾기 · 상세 · "이 내용으로 새 증명서 작성" · 대시보드 요약 · 복구 설정 상태.
//!
//! **이 모듈은 복호화하지 않는다.** 읽는 것은 `repo::certificate::Meta`(암호문 칸을 SELECT 하지 않음)·
//! 경력 사본·출력 이력뿐이다. 주민번호는 `masked_rrn` 만 쓰고, 주소는 다루지 않는다.
//! 정식 출력(재출력)은 이 모듈이 아니라 `service::issuance::for_output` 만 복호화한다.

use std::collections::HashSet;

use rusqlite::Connection;
use serde::Deserialize;

use crate::crypto::keys;
use crate::domain::date;
use crate::error::{AppError, AppResult};
use crate::repo::{certificate as repo, instructor as instructor_repo};
use crate::service::certificate as drafting;

/// 한 번에 보이는 목록 최대 건수 — 넘으면 조건을 좁히라고 알린다.
pub const LIST_LIMIT: i64 = 300;

/// 화면이 보낸 찾기 조건.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    /// 성명·발급번호 일부
    #[serde(default)]
    pub query: String,
    /// 발급일 시작 (YYYY-MM-DD, 비우면 처음부터)
    #[serde(default)]
    pub from: Option<String>,
    /// 발급일 끝 (그날 포함, 비우면 끝까지)
    #[serde(default)]
    pub to: Option<String>,
    /// ALL · ISSUED · VOIDED
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub rows: Vec<repo::Meta>,
    /// `LIST_LIMIT` 보다 많아 잘랐다
    pub truncated: bool,
}

fn opt_day(v: &Option<String>, label: &str) -> AppResult<Option<String>> {
    match v.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(s) => {
            let d = date::parse_iso(s).map_err(|e| AppError::new(e.code(), format!("{label}: {}", e.message())))?;
            Ok(Some(date::to_iso(d)))
        }
    }
}

/// 발급이력 찾기. 성명·발급번호 부분 일치, 발급일 기간(한쪽만도 됨), 상태. 최근 발급순.
pub fn search(conn: &Connection, f: &Filter) -> AppResult<Page> {
    let from = opt_day(&f.from, "발급일 시작")?;
    let to = opt_day(&f.to, "발급일 끝")?;
    if let (Some(a), Some(b)) = (&from, &to) {
        if a > b {
            return Err(AppError::new("HISTORY_RANGE", "발급일 기간의 시작일이 끝 날짜보다 늦습니다."));
        }
    }
    let status = match f.status.as_deref().unwrap_or("ALL") {
        "ALL" | "" => None,
        s @ ("ISSUED" | "VOIDED") => Some(s),
        _ => return Err(AppError::invalid("상태 조건이 올바르지 않습니다.")),
    };
    let mut rows = repo::search(
        conn,
        &repo::ListFilter {
            query: f.query.trim(),
            from: from.as_deref(),
            to: to.as_deref(),
            status,
            limit: LIST_LIMIT + 1,
        },
    )?;
    let truncated = rows.len() as i64 > LIST_LIMIT;
    rows.truncate(LIST_LIMIT as usize);
    Ok(Page { rows, truncated })
}

/// 발급 상세 — 발급 당시 스냅샷(암호문 제외)·경력 사본·출력 이력(시간순).
#[derive(Debug, Clone)]
pub struct Detail {
    pub meta: repo::Meta,
    pub items: Vec<repo::ItemRow>,
    pub outputs: Vec<repo::OutputRow>,
    /// 이 증명서가 복사해 온 원본 (번호, 발급번호)
    pub copied_from: Option<(i64, String)>,
}

pub fn detail(conn: &Connection, id: i64) -> AppResult<Detail> {
    let meta = repo::get_meta(conn, id)?;
    let copied_from = match meta.copied_from_certificate_id {
        Some(cid) => repo::find_meta(conn, cid)?.map(|m| (m.id, m.issue_no)),
        None => None,
    };
    Ok(Detail { items: repo::items(conn, id)?, outputs: repo::outputs(conn, id)?, copied_from, meta })
}

// ---------------------------------------------------------------
// 이 내용으로 새 증명서 작성
// ---------------------------------------------------------------

/// 새 초안의 출발점. **주민번호·주소·발급번호는 없다** — 사용자가 다시 입력한다.
/// 경력은 과거 사본이 아니라 **지금 원장**에서 다시 고른다(사본은 어떤 경력이었는지 참고만).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyPlan {
    pub from_id: i64,
    pub from_issue_no: String,
    pub from_status: String,
    /// 새 초안의 강사 (원래 강사가 없거나 보관되었으면 None — 다시 고른다)
    pub instructor_id: Option<i64>,
    /// 오늘 (YYYY-MM-DD)
    pub issued_on: String,
    pub purpose: String,
    /// FULL · MASK_BACK
    pub rrn_display: String,
    /// 원래 증명서의 경력 중 지금 원장에서 고를 수 있어 선택한 것
    pub selected_career_ids: Vec<i64>,
    /// 원래 증명서에 없던 경력 (선택하지 않음 — 발급일을 바꿔 넣을 수 있게 되어도 저절로 켜지지 않게 모두)
    pub excluded_career_ids: Vec<i64>,
    /// 원래 증명서의 경력 건수
    pub original_count: usize,
    /// 그중 연결하지 못한 건수
    pub unlinked_count: usize,
    /// 화면에 그대로 보일 안내
    pub notices: Vec<String>,
}

pub fn copy_plan(conn: &Connection, id: i64, today: &str) -> AppResult<CopyPlan> {
    let meta = repo::get_meta(conn, id)?;
    let items = repo::items(conn, id)?;
    let original_count = items.len();
    let mut notices = Vec::new();
    if meta.status == "VOIDED" {
        notices.push(format!(
            "취소된 발급 건({})을 참고해 새 증명서를 작성합니다. 취소된 증명서를 되살리거나 재발급하는 것이 아닙니다.",
            meta.issue_no
        ));
    }

    let instructor = match meta.source_instructor_id {
        Some(iid) => instructor_repo::find(conn, iid)?,
        None => None,
    };
    let instructor_id = match &instructor {
        Some(i) if i.archived_at.is_none() => Some(i.id),
        Some(_) => {
            notices.push("원래 강사가 보관되어 새 증명서에 쓸 수 없습니다. 강사를 다시 골라 주세요.".into());
            None
        }
        None => {
            notices.push("원래 강사를 찾을 수 없습니다. 강사를 다시 골라 주세요.".into());
            None
        }
    };

    let (mut selected, mut excluded, mut new_eligible) = (Vec::new(), Vec::new(), 0usize);
    if let Some(iid) = instructor_id {
        let wanted: HashSet<i64> = items.iter().filter_map(|it| it.source_career_id).collect();
        // 보관하지 않은 경력만 나온다 — 보관된 원래 경력은 여기서 빠져 '연결하지 못함' 으로 센다
        for ch in drafting::choices(conn, iid, today)? {
            let eligible = ch.on_issue.is_ok();
            if wanted.contains(&ch.career.id) {
                if eligible {
                    selected.push(ch.career.id); // 오늘 발급일로 넣을 수 없으면 연결하지 못한 것
                }
            } else {
                excluded.push(ch.career.id);
                new_eligible += usize::from(eligible);
            }
        }
    }
    let unlinked_count = original_count.saturating_sub(selected.len());
    if instructor_id.is_some() {
        if unlinked_count > 0 {
            notices.push(format!(
                "기존 발급 경력 {original_count}건 중 현재 원장과 연결 가능한 {}건을 선택했습니다. {unlinked_count}건은 보관되었거나 사용할 수 없습니다.",
                selected.len()
            ));
        } else {
            notices.push(format!("기존 발급 경력 {original_count}건을 모두 현재 원장에서 선택했습니다."));
        }
        if new_eligible > 0 {
            notices.push(format!(
                "기존 발급에 없던 경력 {new_eligible}건은 선택하지 않았습니다. 필요하면 체크해 주세요."
            ));
        }
    }
    notices.push("경력 기간은 현재 원장과 오늘 발급일로 다시 계산합니다. 주민등록번호·주소·발급번호는 새로 입력해 주세요.".into());

    Ok(CopyPlan {
        from_id: meta.id,
        from_issue_no: meta.issue_no,
        from_status: meta.status,
        instructor_id,
        issued_on: today.to_string(),
        purpose: meta.purpose,
        rrn_display: meta.rrn_display,
        selected_career_ids: selected,
        excluded_career_ids: excluded,
        original_count,
        unlinked_count,
        notices,
    })
}

// ---------------------------------------------------------------
// 대시보드 · 복구 설정 상태
// ---------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Recent {
    pub issued_count: i64,
    pub voided_count: i64,
    pub issued: Vec<repo::Meta>,
    pub voided: Vec<repo::Meta>,
}

pub fn recent(conn: &Connection, limit: i64) -> AppResult<Recent> {
    let (issued_count, voided_count) = repo::counts(conn)?;
    Ok(Recent {
        issued_count,
        voided_count,
        issued: repo::recent_issued(conn, limit)?,
        voided: repo::recent_voided(conn, limit)?,
    })
}

/// 암호화 자료를 다른 PC·계정으로 옮길 수 있는가. 키 값은 보지 않고 key_store 의 칸 유무만 센다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryState {
    /// 아직 암호 키가 없다(발급 전)
    NoData,
    /// 복구 사본이 없는 키가 있다 — 이 PC·이 Windows 사용자만 열 수 있다
    NotSet,
    /// 모든 키에 복구 사본이 있다 (Phase 8 에서 채운다)
    Ready,
}

impl RecoveryState {
    pub fn code(self) -> &'static str {
        match self {
            RecoveryState::NoData => "NO_DATA",
            RecoveryState::NotSet => "NOT_SET",
            RecoveryState::Ready => "READY",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            RecoveryState::NoData => {
                "암호화 자료의 이동용 복구 설정이 아직 되어 있지 않습니다. 발급을 시작하면 주민등록번호와 주소는 이 PC의 현재 Windows 사용자 계정에서만 열 수 있게 저장됩니다."
            }
            RecoveryState::NotSet => {
                "암호화 자료의 이동용 복구 설정이 아직 되어 있지 않습니다. 현재 발급 기록의 주민등록번호와 주소는 이 PC의 현재 Windows 사용자 계정에서만 열 수 있습니다."
            }
            RecoveryState::Ready => "암호화 자료의 이동용 복구 설정이 되어 있습니다.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recovery {
    pub state: RecoveryState,
    pub certificates: i64,
}

pub fn recovery(conn: &Connection) -> AppResult<Recovery> {
    let (keys, missing) = keys::recovery_counts(conn)?;
    let state = if keys == 0 {
        RecoveryState::NoData
    } else if missing > 0 {
        RecoveryState::NotSet
    } else {
        RecoveryState::Ready
    };
    let (issued, voided) = repo::counts(conn)?;
    Ok(Recovery { state, certificates: issued + voided })
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod history_tests;
