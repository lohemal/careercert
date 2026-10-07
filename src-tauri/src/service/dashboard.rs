//! 대시보드 요약과 "확인 필요".
//!
//! **지금 자료만으로 객관적으로 가릴 수 있는 것만** 보인다. 재직중 경력에는 예정 종료일이 없으므로
//! "기간이 지났는데 재직중" 같은 판단은 하지 않는다(추측이 된다). 알리기만 하고 아무것도 바꾸지 않는다.
//! 보관한 강사·경력은 보지 않는다.

use std::collections::BTreeMap;

use rusqlite::Connection;

use super::career::today;
use crate::domain::career::{planned_end_passed, Career, Term};
use crate::domain::{date, overlap};
use crate::error::AppResult;
use crate::repo::{career as career_repo, overlap_ack};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckItem {
    pub instructor_id: i64,
    pub instructor_name: String,
    pub distinguisher: String,
    /// 무엇이 걸렸는지 한 줄
    pub detail: String,
    /// 기간 겹침 항목만 — '정상 경력으로 확인' 에 쓰는 두 경력과 지금 지문
    pub overlap: Option<OverlapRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlapRef {
    pub career_a_id: i64,
    pub career_b_id: i64,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// PLANNED_END_PASSED · DUPLICATE_NAME · ACTIVE_STARTS_LATER · ENDS_LATER · OVERLAP
    pub kind: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub items: Vec<CheckItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overview {
    /// 보관하지 않은 강사
    pub instructors: i64,
    /// 보관하지 않은 강사의 보관하지 않은 재직중 경력
    pub active_careers: i64,
    /// 걸린 것이 있는 확인 항목만
    pub checks: Vec<Check>,
}

struct Who {
    name: String,
    distinguisher: String,
}

pub fn overview(conn: &Connection, now: &str) -> AppResult<Overview> {
    let today = today(now)?;

    // 보관하지 않은 강사
    let mut who: BTreeMap<i64, Who> = BTreeMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, distinguisher FROM instructors WHERE archived_at IS NULL ORDER BY name, id",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?)))?;
        for row in rows {
            let (id, name, distinguisher) = row?;
            who.insert(id, Who { name, distinguisher });
        }
    }

    // 그 강사들의 보관하지 않은 경력
    let mut careers: BTreeMap<i64, Vec<Career>> = BTreeMap::new();
    for id in who.keys() {
        careers.insert(*id, career_repo::list_by_instructor(conn, *id, false)?);
    }

    let item = |id: i64, detail: String| CheckItem {
        instructor_id: id,
        instructor_name: who[&id].name.clone(),
        distinguisher: who[&id].distinguisher.clone(),
        detail,
        overlap: None,
    };
    let line = |c: &Career| format!("{} · {}", c.fields.program_name, c.period());

    // 1) 같은 이름인데 구분 메모가 비어 있는 강사 — 발급 때 잘못 고를 수 있다
    let mut by_name: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for (id, w) in &who {
        by_name.entry(w.name.as_str()).or_default().push(*id);
    }
    let duplicate_name: Vec<CheckItem> = by_name
        .values()
        .filter(|ids| ids.len() > 1)
        .flatten()
        .filter(|id| who[id].distinguisher.trim().is_empty())
        .map(|id| item(*id, format!("같은 이름 {}명", by_name[who[id].name.as_str()].len())))
        .collect();

    let acked = overlap_ack::fingerprints(conn)?;
    let mut planned_passed = Vec::new();
    let mut starts_later = Vec::new();
    let mut ends_later = Vec::new();
    let mut overlap = Vec::new();
    for (id, list) in &careers {
        for c in list {
            // 0) 재직중인데 예정 종료일이 지났다 — 종료 처리를 잊었을 수 있다 (저절로 끝내지 않는다)
            if planned_end_passed(&c.fields, today) {
                let planned = c.fields.planned_end_date.map(date::display).unwrap_or_default();
                planned_passed.push(item(*id, format!("{} · 예정 종료일 {planned}", line(c))));
            }
            // 2) 시작일이 오늘 이후인 재직중 경력
            if c.fields.term == Term::Active && c.fields.start_date > today {
                starts_later.push(item(*id, line(c)));
            }
            // 3) 종료일이 오늘 이후인 종료 경력 (미리 입력한 계약 종료)
            if let Some(end) = c.fields.term.end_date() {
                if end > today {
                    ends_later.push(item(*id, line(c)));
                }
            }
        }
        // 4) 같은 강사의 경력 기간이 겹친다 (두 프로그램을 함께 맡았을 수도 있다 — 확인만).
        //    담당자가 그 한 쌍을 '정상 경력' 으로 확인했고 지문(기간)이 그대로면 보이지 않는다.
        for (i, a) in list.iter().enumerate() {
            for b in &list[i + 1..] {
                if !overlap::overlaps(a, b) {
                    continue;
                }
                let pair = overlap::pair(a, b);
                if acked.contains(&pair.fingerprint) {
                    continue;
                }
                overlap.push(CheckItem {
                    overlap: Some(OverlapRef { career_a_id: a.id, career_b_id: b.id, fingerprint: pair.fingerprint }),
                    ..item(*id, format!("{} / {}", line(a), line(b)))
                });
            }
        }
    }

    let checks = [
        Check {
            kind: "PLANNED_END_PASSED",
            title: "예정 종료일이 지난 재직중 경력",
            description: "예정 종료일이 지났습니다. 종료 여부를 확인하세요. 프로그램이 저절로 종료하지 않습니다.",
            items: planned_passed,
        },
        Check {
            kind: "DUPLICATE_NAME",
            title: "동명이인 구분 메모 없음",
            description: "같은 이름의 강사가 있는데 구분 메모가 비어 있습니다. 발급할 때 잘못 고르지 않도록 메모를 적어 두세요.",
            items: duplicate_name,
        },
        Check {
            kind: "ACTIVE_STARTS_LATER",
            title: "시작일이 오늘 이후인 재직중 경력",
            description: "아직 시작하지 않은 경력입니다. 날짜를 잘못 적지 않았는지 확인하세요.",
            items: starts_later,
        },
        Check {
            kind: "ENDS_LATER",
            title: "종료일이 오늘 이후인 경력",
            description: "종료 예정일을 미리 입력한 경력입니다.",
            items: ends_later,
        },
        Check {
            kind: "OVERLAP",
            title: "기간이 겹치는 경력",
            description: "같은 강사의 경력 기간이 겹칩니다. 같은 기간에 두 프로그램을 함께 맡은 실제 경력이면 [정상 경력으로 확인] 을 누르세요. 기간이 바뀌면 다시 나타납니다.",
            items: overlap,
        },
    ]
    .into_iter()
    .filter(|c| !c.items.is_empty())
    .collect();

    let active_careers = careers
        .values()
        .flatten()
        .filter(|c| c.fields.term == Term::Active)
        .count() as i64;

    Ok(Overview {
        instructors: who.len() as i64,
        active_careers,
        checks,
    })
}

#[cfg(test)]
#[path = "dashboard_tests.rs"]
pub(crate) mod dashboard_tests;

#[cfg(test)]
#[path = "overlap_ack_tests.rs"]
mod overlap_ack_tests;
