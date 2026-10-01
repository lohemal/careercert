//! `school_settings` (행 하나, id = 1).

use rusqlite::{params, Connection};

use crate::domain::settings::{prepare, SchoolSettings, SettingsInput};
use crate::error::AppResult;

pub fn get(conn: &Connection) -> AppResult<SchoolSettings> {
    Ok(conn.query_row(
        "SELECT school_name, certificate_title, issuer_title, department,
                manager_name, phone, default_purpose, updated_at
           FROM school_settings WHERE id = 1",
        [],
        |r| {
            Ok(SchoolSettings {
                school_name: r.get(0)?,
                certificate_title: r.get(1)?,
                issuer_title: r.get(2)?,
                department: r.get(3)?,
                manager_name: r.get(4)?,
                phone: r.get(5)?,
                default_purpose: r.get(6)?,
                updated_at: r.get(7)?,
            })
        },
    )?)
}

/// 저장 결과.
#[derive(Debug, Clone)]
pub struct Saved {
    pub settings: SchoolSettings,
    /// 발급자 표기가 비어 있어 학교명으로 채웠는가 (화면이 알린다)
    pub issuer_filled: bool,
}

/// 화면에서 받은 값을 **그대로** 저장한다. 정리·검사는 `domain::settings::prepare` 한 곳.
///
/// 지금 DB 에 있는 값을 보고 무엇을 바꾸지 않는다 — 특히 학교명이 바뀌었다고 발급자 표기를
/// 고치지 않는다. 화면이 보낸 발급자 표기가 곧 저장되는 값이다(비어 있을 때만 채운다).
pub fn save(conn: &Connection, raw: SettingsInput, now: &str) -> AppResult<Saved> {
    let p = prepare(raw)?;
    let v = &p.input;
    conn.execute(
        "UPDATE school_settings
            SET school_name = ?1, certificate_title = ?2, issuer_title = ?3, department = ?4,
                manager_name = ?5, phone = ?6, default_purpose = ?7, updated_at = ?8
          WHERE id = 1",
        params![
            v.school_name,
            v.certificate_title,
            v.issuer_title,
            v.department,
            v.manager_name,
            v.phone,
            v.default_purpose,
            now,
        ],
    )?;
    Ok(Saved {
        settings: get(conn)?,
        issuer_filled: p.issuer_filled,
    })
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
