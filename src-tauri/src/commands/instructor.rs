//! 강사 — 화면에 보내는 모양과 명령.

use serde::Serialize;
use tauri::State;

use super::now;
use crate::domain::instructor::{Instructor, InstructorInput};
use crate::error::AppResult;
use crate::repo::instructor::{Filter, Scope, Summary};
use crate::service::instructor as service;
use crate::AppState;

/// 화면에 보내는 강사. 주민번호·주소는 없다(원장에 없으므로).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructorView {
    pub id: i64,
    pub name: String,
    pub distinguisher: String,
    pub phone: String,
    pub memo: String,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Instructor> for InstructorView {
    fn from(i: Instructor) -> Self {
        InstructorView {
            archived: i.is_archived(),
            id: i.id,
            name: i.name,
            distinguisher: i.distinguisher,
            phone: i.phone,
            memo: i.memo,
            archived_at: i.archived_at,
            created_at: i.created_at,
            updated_at: i.updated_at,
        }
    }
}

/// 목록 한 줄.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructorRow {
    #[serde(flatten)]
    pub instructor: InstructorView,
    /// 보관하지 않은 경력 수
    pub careers: i64,
    pub active_careers: i64,
    pub programs: Vec<String>,
    /// 지금 목록에 같은 이름이 또 있다 — 화면이 구분 메모를 눈에 띄게 보인다
    pub same_name: bool,
}

fn rows(list: Vec<Summary>) -> Vec<InstructorRow> {
    let mut count = std::collections::HashMap::<String, usize>::new();
    for s in &list {
        *count.entry(s.instructor.name.clone()).or_default() += 1;
    }
    list.into_iter()
        .map(|s| InstructorRow {
            same_name: count[&s.instructor.name] > 1,
            careers: s.careers,
            active_careers: s.active_careers,
            programs: s.programs,
            instructor: s.instructor.into(),
        })
        .collect()
}

#[tauri::command]
pub fn instructor_search(state: State<'_, AppState>, query: String, scope: Scope) -> AppResult<Vec<InstructorRow>> {
    let list = state.db.read(|c| service::search(c, &Filter { query, scope }))?;
    Ok(rows(list))
}

#[tauri::command]
pub fn instructor_get(state: State<'_, AppState>, id: i64) -> AppResult<InstructorView> {
    Ok(state.db.read(|c| service::get(c, id))?.into())
}

#[tauri::command]
pub fn instructor_create(state: State<'_, AppState>, input: InstructorInput) -> AppResult<InstructorView> {
    let now = now();
    Ok(state.db.write(|c| service::create(c, input, &now))?.into())
}

#[tauri::command]
pub fn instructor_update(state: State<'_, AppState>, id: i64, input: InstructorInput) -> AppResult<InstructorView> {
    let now = now();
    Ok(state.db.write(|c| service::update(c, id, input, &now))?.into())
}

#[tauri::command]
pub fn instructor_archive(state: State<'_, AppState>, id: i64) -> AppResult<InstructorView> {
    let now = now();
    Ok(state.db.write(|c| service::archive(c, id, &now))?.into())
}

#[tauri::command]
pub fn instructor_unarchive(state: State<'_, AppState>, id: i64) -> AppResult<InstructorView> {
    let now = now();
    Ok(state.db.write(|c| service::unarchive(c, id, &now))?.into())
}
