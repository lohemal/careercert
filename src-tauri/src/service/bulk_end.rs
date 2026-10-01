//! 재직중 경력 일괄 종료 (설계안 §15.2).
//!
//! **프로그램은 아무것도 저절로 끝내지 않는다.** 담당자가 대상을 고르고(기본 0건), 종료일·사유를
//! 정하고, 미리보기를 보고, 승인해야만 바뀐다.
//!
//! ```text
//!   candidates ─▶ (사람이 고름) ─▶ preview ─▶ (사람이 확인) ─▶ apply
//!                                    │                          │
//!                                    └── state_key ─────────────┘  미리보기 뒤 자료가 바뀌었으면 거절
//! ```
//!
//! * `apply` 는 **트랜잭션 안에서 미리보기를 다시 만든다.** 같은 함수, 같은 판정이므로 화면이 본 것과
//!   적용되는 것이 다를 수 없다. state_key 가 다르면 `BULK_STALE`.
//! * 한 건이라도 끝낼 수 없으면(이미 종료·보관·시작일보다 이른 종료일 …) **아무것도 바꾸지 않는다.**
//! * 미래 종료일은 단건 종료와 같은 확인(`confirm_future`)을 받는다.
//! * 적용 직전 백업(`before_bulk_end_`)은 트랜잭션 밖에서 부르는 쪽(command)이 뜬다.

use std::collections::{BTreeSet, HashMap};

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Deserialize;

use super::career::{end_summary, today};
use crate::domain::audit::Action;
use crate::domain::career::{self, Career, EndReason, Term};
use crate::domain::date;
use crate::error::{AppError, AppResult};
use crate::repo::{audit, career as repo, instructor as instructor_repo};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkEndRequest {
    pub career_ids: Vec<i64>,
    /// `YYYY-MM-DD`
    pub end_date: String,
    pub reason: EndReason,
}

/// 고를 수 있는 경력 한 줄.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub career: Career,
    pub instructor_name: String,
    pub distinguisher: String,
}

/// 일괄 종료 대상이 될 수 있는 경력 — 재직중이고, 경력도 강사도 보관되지 않은 것.
/// 강사 이름 차례, 같은 강사는 시작일 차례.
pub fn candidates(conn: &Connection) -> AppResult<Vec<Candidate>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, i.name, i.distinguisher
           FROM careers c JOIN instructors i ON i.id = c.instructor_id
          WHERE c.status = 'ACTIVE' AND c.archived_at IS NULL AND i.archived_at IS NULL
          ORDER BY i.name, i.id, c.start_date, c.id",
    )?;
    let rows: Vec<(i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    rows.into_iter()
        .map(|(id, instructor_name, distinguisher)| {
            Ok(Candidate {
                career: repo::get(conn, id)?,
                instructor_name,
                distinguisher,
            })
        })
        .collect()
}

/// 미리보기 한 줄.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewItem {
    pub career_id: i64,
    pub instructor_id: Option<i64>,
    pub instructor_name: String,
    pub distinguisher: String,
    pub program_name: String,
    /// `2026.03.04 ~ 현재`
    pub before: String,
    /// `2026.03.04 ~ 2027.02.12` — 끝낼 수 없으면 None
    pub after: Option<String>,
    /// 끝낼 수 없는 까닭
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub items: Vec<PreviewItem>,
    pub end_date: NaiveDate,
    pub reason: EndReason,
    /// 종료일이 오늘보다 뒤
    pub future: bool,
    /// 끝낼 수 없는 줄이 하나도 없다
    pub ok: bool,
    /// 미리보기 당시 자료의 지문. apply 가 같은지 본다.
    pub state_key: String,
}

impl Preview {
    pub fn problems(&self) -> usize {
        self.items.iter().filter(|i| i.problem.is_some()).count()
    }
}

/// 미리보기. 아무것도 쓰지 않는다.
pub fn preview(conn: &Connection, req: &BulkEndRequest, now: &str) -> AppResult<Preview> {
    let ids: BTreeSet<i64> = req.career_ids.iter().copied().collect();
    if ids.is_empty() {
        return Err(AppError::new("BULK_EMPTY", "종료할 경력을 하나 이상 골라 주세요."));
    }
    let end_date = date::parse_iso(&req.end_date)
        .map_err(|e| AppError::new(e.code(), format!("종료일: {}", e.message())))?;
    let today = today(now)?;

    let mut items = Vec::with_capacity(ids.len());
    let mut key = Fnv::new();
    key.add(&date::to_iso(end_date));
    key.add(req.reason.code());
    let mut instructors: HashMap<i64, (String, String, bool)> = HashMap::new();

    for id in ids {
        key.add(&id.to_string());
        let Some(c) = repo::find(conn, id)? else {
            key.add("missing");
            items.push(PreviewItem {
                career_id: id,
                instructor_id: None,
                instructor_name: String::new(),
                distinguisher: String::new(),
                program_name: String::new(),
                before: String::new(),
                after: None,
                problem: Some("경력을 찾을 수 없습니다(지워졌거나 바뀌었습니다).".into()),
            });
            continue;
        };
        if !instructors.contains_key(&c.instructor_id) {
            let i = instructor_repo::get(conn, c.instructor_id)?;
            instructors.insert(c.instructor_id, (i.name, i.distinguisher, i.archived_at.is_some()));
        }
        let (name, distinguisher, owner_archived) = instructors[&c.instructor_id].clone();
        // 지문: 이 경력의 내용과 그 강사의 지금 상태. updated_at 은 초 단위라 같은 초 안의
        // 두 번째 수정을 놓칠 수 있어 내용 자체도 넣는다.
        key.add(&c.updated_at);
        key.add(&format!("{:?}", c.fields));
        key.add(c.archived_at.as_deref().unwrap_or("-"));
        key.add(if owner_archived { "owner-archived" } else { "owner-ok" });

        let outcome: Result<Term, String> = if c.is_archived() {
            Err("보관된 경력입니다.".into())
        } else if owner_archived {
            Err("보관된 강사의 경력입니다.".into())
        } else {
            career::end(&c.fields, &req.end_date, req.reason).map_err(|e| e.user_message)
        };
        items.push(PreviewItem {
            career_id: c.id,
            instructor_id: Some(c.instructor_id),
            instructor_name: name,
            distinguisher,
            program_name: c.fields.program_name.clone(),
            before: c.period(),
            after: outcome
                .as_ref()
                .ok()
                .map(|t| date::display_period(c.fields.start_date, t.end_date())),
            problem: outcome.err(),
        });
    }

    let ok = items.iter().all(|i| i.problem.is_none());
    Ok(Preview {
        items,
        end_date,
        reason: req.reason,
        future: end_date > today,
        ok,
        state_key: key.finish(),
    })
}

/// 적용 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub ended: usize,
}

/// 미리보기와 같은지 다시 확인하고 **전부** 끝내거나 **하나도** 끝내지 않는다.
/// `Db::write` 안(한 트랜잭션)에서 부른다.
pub fn apply(
    conn: &Connection,
    req: &BulkEndRequest,
    state_key: &str,
    confirm_future: bool,
    now: &str,
) -> AppResult<Applied> {
    let p = check(conn, req, state_key, confirm_future, now)?;
    let term = Term::Ended {
        end_date: p.end_date,
        reason: p.reason,
    };
    for item in &p.items {
        repo::set_term(conn, item.career_id, &term, now)?;
        audit::add(
            conn,
            now,
            Action::CareerEnd,
            Some(item.career_id),
            &format!("{} (일괄 종료)", end_summary(&term)),
        )?;
    }
    audit::add(
        conn,
        now,
        Action::BulkEnd,
        None,
        &format!("재직중 경력 {}건 일괄 종료 · {}", p.items.len(), end_summary(&term)),
    )?;
    Ok(Applied { ended: p.items.len() })
}

/// apply 전에 통과해야 하는 검사. 부르는 쪽이 백업을 뜨기 전에도 한 번 부른다
/// (어차피 거절될 작업으로 백업을 쌓지 않으려고).
pub fn check(
    conn: &Connection,
    req: &BulkEndRequest,
    state_key: &str,
    confirm_future: bool,
    now: &str,
) -> AppResult<Preview> {
    let p = preview(conn, req, now)?;
    if p.state_key != state_key {
        return Err(AppError::new(
            "BULK_STALE",
            "미리보기 뒤에 자료가 바뀌었습니다. 아무것도 바꾸지 않았습니다. 다시 미리보기 해 주세요.",
        ));
    }
    if !p.ok {
        return Err(AppError::new(
            "BULK_INVALID",
            format!(
                "종료할 수 없는 경력이 {}건 있어 아무것도 바꾸지 않았습니다. 해당 경력을 빼고 다시 미리보기 해 주세요.",
                p.problems()
            ),
        ));
    }
    if p.future && !confirm_future {
        return Err(AppError::new(
            "FUTURE_END_UNCONFIRMED",
            format!(
                "종료일이 오늘 이후입니다. {}로 {}건을 종료 처리하시겠습니까?",
                date::display(p.end_date),
                p.items.len()
            ),
        ));
    }
    Ok(p)
}

/// 64비트 FNV-1a — 미리보기 지문용(보안용 아님). 항목 사이에 구분 바이트를 넣는다.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn add(&mut self, s: &str) {
        for b in s.bytes().chain(std::iter::once(0x1f)) {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn finish(&self) -> String {
        format!("{:016x}", self.0)
    }
}

/// 일괄 종료에 쓰인 경력 수 (시험·확인용).
#[cfg(test)]
pub fn count_ended_on(conn: &Connection, end_date: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM careers WHERE status = 'ENDED' AND end_date = ?1",
        rusqlite::params![end_date],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
#[path = "bulk_end_tests.rs"]
mod bulk_end_tests;
