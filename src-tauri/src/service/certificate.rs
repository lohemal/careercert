//! 증명서 작성 — 고를 수 있는 경력, 내용 만들기.
//!
//! **읽기만 한다.** Phase 4 에서는 발급 기록도, 작성 중인 값도 DB 에 쓰지 않는다(D6).
//! 주민번호·주소는 이 함수들을 지나 화면으로 돌아갈 뿐 어디에도 남지 않는다 —
//! 로그·변경 기록·오류 문구에 넣지 않는다.

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Deserialize;

use super::career::today;
use crate::domain::career::Career;
use crate::domain::certificate::{self, Built, Ineligible, IssueInput};
use crate::domain::date;
use crate::error::{AppError, AppResult};
use crate::repo::{career as career_repo, instructor as instructor_repo, settings as settings_repo};

/// 작성 화면 2단계의 한 줄.
#[derive(Debug, Clone)]
pub struct Choice {
    pub career: Career,
    /// 발급일 기준 끝(`Ok(None)` = 현재) 또는 넣을 수 없는 까닭
    pub on_issue: Result<Option<NaiveDate>, Ineligible>,
}

/// 이 강사의 **보관하지 않은** 경력과, 이 발급일의 증명서에 넣을 수 있는지. 시작일 차례.
pub fn choices(conn: &Connection, instructor_id: i64, issued_on: &str) -> AppResult<Vec<Choice>> {
    let issued_on = date::parse_iso(issued_on)
        .map_err(|e| AppError::new(e.code(), format!("발급일: {}", e.message())))?;
    instructor_repo::get(conn, instructor_id)?;
    Ok(career_repo::list_by_instructor(conn, instructor_id, false)?
        .into_iter()
        .map(|c| Choice {
            on_issue: certificate::eligibility(&c, issued_on),
            career: c,
        })
        .collect())
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareRequest {
    pub instructor_id: i64,
    pub career_ids: Vec<i64>,
    pub issue: IssueInput,
}

impl std::fmt::Debug for PrepareRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrepareRequest")
            .field("instructor_id", &self.instructor_id)
            .field("career_ids", &self.career_ids)
            .field("issue", &self.issue) // IssueInput 의 Debug 가 주민번호·주소를 가린다
            .finish()
    }
}

/// 지금 자료(강사·경력·학교 설정)로 증명서 내용을 만든다. **아무것도 쓰지 않는다.**
pub fn prepare(conn: &Connection, req: PrepareRequest, now: &str) -> AppResult<Built> {
    let instructor = instructor_repo::get(conn, req.instructor_id)?;
    let mut careers = Vec::with_capacity(req.career_ids.len());
    for id in &req.career_ids {
        let c = career_repo::find(conn, *id)?.ok_or_else(|| {
            AppError::new("CERT_CAREER_MISSING", "고른 경력을 찾을 수 없습니다. 화면을 새로 고쳐 주세요.")
        })?;
        careers.push(c);
    }
    let school = settings_repo::get(conn)?;
    certificate::build(&instructor, &careers, req.issue, &school, today(now)?)
}

#[cfg(test)]
#[path = "certificate_tests.rs"]
mod certificate_tests;
