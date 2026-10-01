//! 학교 기본정보 규칙.
//!
//! 지키는 것
//!   * 입력은 **앞뒤 공백만** 정리한다. 가운데 글자는 바꾸지 않는다.
//!   * 학교명과 발급자 표기는 서로 독립이다. 발급자 표기가 **비어 있을 때만** 학교명으로
//!     채운다. 값이 있는 발급자 표기는 학교명이 바뀌어도 건드리지 않는다.
//!   * 문서 제목·기본 용도는 증명서에 반드시 찍히므로 비워 둘 수 없다.
//!     나머지는 비워 두고 저장할 수 있다 — 대신 `missing` 이 무엇이 빠졌는지 알린다.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// 문서 제목 기본값 (기존 양식의 제목, 결정 D1)
pub const DEFAULT_TITLE: &str = "방과후학교 개인위탁 외부강사 활동 확인서";

/// 용도 기본값
pub const DEFAULT_PURPOSE: &str = "기관제출";

/// 한 칸에 받을 수 있는 최대 글자 수. 양식 한 줄에 들어가지 않는 길이는 입력 실수로 본다.
pub const MAX_LEN: usize = 100;

/// 저장된 학교 기본정보.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolSettings {
    pub school_name: String,
    pub certificate_title: String,
    pub issuer_title: String,
    pub department: String,
    pub manager_name: String,
    pub phone: String,
    pub default_purpose: String,
    /// 한 번도 저장하지 않았으면 None
    pub updated_at: Option<String>,
}

/// 화면에서 받은 값.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    pub school_name: String,
    pub certificate_title: String,
    pub issuer_title: String,
    pub department: String,
    pub manager_name: String,
    pub phone: String,
    pub default_purpose: String,
}

/// 칸 이름 — 오류 문구와 "빠진 항목" 안내에 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldKey {
    SchoolName,
    CertificateTitle,
    IssuerTitle,
    Department,
    ManagerName,
    Phone,
    DefaultPurpose,
}

impl FieldKey {
    pub fn label(self) -> &'static str {
        match self {
            FieldKey::SchoolName => "학교명",
            FieldKey::CertificateTitle => "문서 제목",
            FieldKey::IssuerTitle => "발급자 표기",
            FieldKey::Department => "담당부서",
            FieldKey::ManagerName => "담당자",
            FieldKey::Phone => "전화번호",
            FieldKey::DefaultPurpose => "기본 용도",
        }
    }
}

/// 학교명으로 만든 발급자 표기 제안. `○○초등학교` → `○○초등학교장`.
/// 학교명이 비어 있으면 제안하지 않는다.
pub fn suggest_issuer(school_name: &str) -> Option<String> {
    let name = school_name.trim();
    if name.is_empty() {
        None
    } else {
        Some(format!("{name}장"))
    }
}

/// 정리·검사를 마친 저장할 값과, 발급자 표기를 프로그램이 채웠는지.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub input: SettingsInput,
    pub issuer_filled: bool,
}

/// 저장 전에 거치는 단 하나의 길.
///
/// 1. 모든 칸 앞뒤 공백 정리 (가운데는 그대로)
/// 2. 글자 수 검사
/// 3. 문서 제목·기본 용도가 비었으면 거절
/// 4. 발급자 표기가 **비어 있고** 학교명이 있으면 제안값으로 채움 — 값이 있으면 절대 바꾸지 않음
pub fn prepare(raw: SettingsInput) -> AppResult<Prepared> {
    let mut v = SettingsInput {
        school_name: raw.school_name.trim().to_string(),
        certificate_title: raw.certificate_title.trim().to_string(),
        issuer_title: raw.issuer_title.trim().to_string(),
        department: raw.department.trim().to_string(),
        manager_name: raw.manager_name.trim().to_string(),
        phone: raw.phone.trim().to_string(),
        default_purpose: raw.default_purpose.trim().to_string(),
    };

    for (key, value) in fields(&v) {
        if value.chars().count() > MAX_LEN {
            return Err(AppError::invalid(format!(
                "{}은(는) {MAX_LEN}자 이내로 입력해 주세요.",
                key.label()
            )));
        }
    }
    if v.certificate_title.is_empty() {
        return Err(AppError::invalid(
            "문서 제목은 비워 둘 수 없습니다. [기본값으로] 를 누르면 기존 양식의 제목으로 돌아갑니다.",
        ));
    }
    if v.default_purpose.is_empty() {
        return Err(AppError::invalid(
            "기본 용도는 비워 둘 수 없습니다. 보통 '기관제출' 을 씁니다.",
        ));
    }

    let mut issuer_filled = false;
    if v.issuer_title.is_empty() {
        if let Some(s) = suggest_issuer(&v.school_name) {
            v.issuer_title = s;
            issuer_filled = true;
        }
    }

    Ok(Prepared {
        input: v,
        issuer_filled,
    })
}

/// 증명서에 찍히는데 아직 비어 있는 칸. 비어 있으면 설정이 끝난 것이다.
pub fn missing(s: &SchoolSettings) -> Vec<FieldKey> {
    [
        (FieldKey::SchoolName, &s.school_name),
        (FieldKey::IssuerTitle, &s.issuer_title),
        (FieldKey::Department, &s.department),
        (FieldKey::ManagerName, &s.manager_name),
        (FieldKey::Phone, &s.phone),
        (FieldKey::CertificateTitle, &s.certificate_title),
        (FieldKey::DefaultPurpose, &s.default_purpose),
    ]
    .into_iter()
    .filter(|(_, v)| v.trim().is_empty())
    .map(|(k, _)| k)
    .collect()
}

fn fields(v: &SettingsInput) -> [(FieldKey, &str); 7] {
    [
        (FieldKey::SchoolName, &v.school_name),
        (FieldKey::CertificateTitle, &v.certificate_title),
        (FieldKey::IssuerTitle, &v.issuer_title),
        (FieldKey::Department, &v.department),
        (FieldKey::ManagerName, &v.manager_name),
        (FieldKey::Phone, &v.phone),
        (FieldKey::DefaultPurpose, &v.default_purpose),
    ]
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
