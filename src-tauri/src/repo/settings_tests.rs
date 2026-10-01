use super::*;
use crate::db::testutil::tmp_dir;
use crate::db::{Db, DB_FILE};
use crate::domain::settings::{DEFAULT_PURPOSE, DEFAULT_TITLE};

const NOW: &str = "2026-10-01T09:00:00";

fn input() -> SettingsInput {
    SettingsInput {
        school_name: "○○초등학교".into(),
        certificate_title: DEFAULT_TITLE.into(),
        issuer_title: "".into(),
        department: "방과후학교".into(),
        manager_name: "가상담당 (인)".into(),
        phone: "000-0000-0000".into(),
        default_purpose: DEFAULT_PURPOSE.into(),
    }
}

/// 지금 저장된 값을 화면이 보내는 모양으로 되돌린다 — "화면에서 한 칸만 고쳐 저장" 흉내
fn as_input(s: &SchoolSettings) -> SettingsInput {
    SettingsInput {
        school_name: s.school_name.clone(),
        certificate_title: s.certificate_title.clone(),
        issuer_title: s.issuer_title.clone(),
        department: s.department.clone(),
        manager_name: s.manager_name.clone(),
        phone: s.phone.clone(),
        default_purpose: s.default_purpose.clone(),
    }
}

#[test]
fn 처음에는_기본값만_있다() {
    let db = Db::memory();
    let s = db.read(get).unwrap();
    assert_eq!(s.certificate_title, DEFAULT_TITLE);
    assert_eq!(s.default_purpose, DEFAULT_PURPOSE);
    assert_eq!(s.school_name, "");
    assert_eq!(s.issuer_title, "");
    assert_eq!(s.updated_at, None);
}

#[test]
fn 저장한_값을_그대로_다시_읽는다() {
    let db = Db::memory();
    let saved = db.write(|c| save(c, input(), NOW)).unwrap();
    let read = db.read(get).unwrap();
    assert_eq!(saved.settings, read);
    assert_eq!(read.school_name, "○○초등학교");
    assert_eq!(read.issuer_title, "○○초등학교장");
    assert_eq!(read.department, "방과후학교");
    assert_eq!(read.manager_name, "가상담당 (인)");
    assert_eq!(read.phone, "000-0000-0000");
    assert_eq!(read.updated_at.as_deref(), Some(NOW));
    assert!(saved.issuer_filled);
}

#[test]
fn 고친_값이_반영된다() {
    let db = Db::memory();
    db.write(|c| save(c, input(), NOW)).unwrap();

    let mut v = as_input(&db.read(get).unwrap());
    v.manager_name = "새담당 (인)".into();
    v.certificate_title = "방과후학교 강사 경력증명서".into();
    v.default_purpose = "취업용".into();
    db.write(|c| save(c, v, "2026-10-02T10:00:00")).unwrap();

    let s = db.read(get).unwrap();
    assert_eq!(s.manager_name, "새담당 (인)");
    assert_eq!(s.certificate_title, "방과후학교 강사 경력증명서");
    assert_eq!(s.default_purpose, "취업용");
    assert_eq!(s.department, "방과후학교", "고치지 않은 칸은 그대로");
    assert_eq!(s.updated_at.as_deref(), Some("2026-10-02T10:00:00"));
}

#[test]
fn 직접_고친_발급자_표기는_학교명을_바꿔도_덮어쓰지_않는다() {
    let db = Db::memory();
    db.write(|c| save(c, input(), NOW)).unwrap();

    // 사용자가 발급자 표기를 직접 고친다
    let mut v = as_input(&db.read(get).unwrap());
    v.issuer_title = "○○초등학교장 직무대리".into();
    db.write(|c| save(c, v, NOW)).unwrap();

    // 그다음 학교명만 바꾼다
    let mut v = as_input(&db.read(get).unwrap());
    v.school_name = "△△초등학교".into();
    let saved = db.write(|c| save(c, v, NOW)).unwrap();

    let s = db.read(get).unwrap();
    assert_eq!(s.school_name, "△△초등학교");
    assert_eq!(s.issuer_title, "○○초등학교장 직무대리");
    assert!(!saved.issuer_filled);
}

#[test]
fn 자동으로_채운_발급자_표기도_학교명을_바꿨다고_따라_바뀌지_않는다() {
    // 따라 바꿀지는 화면이 사용자에게 묻는다 — 저장소는 받은 값을 그대로 둔다
    let db = Db::memory();
    db.write(|c| save(c, input(), NOW)).unwrap();
    let mut v = as_input(&db.read(get).unwrap());
    v.school_name = "△△초등학교".into();
    db.write(|c| save(c, v, NOW)).unwrap();
    assert_eq!(db.read(get).unwrap().issuer_title, "○○초등학교장");
}

#[test]
fn 발급자_표기를_비우고_저장하면_지금_학교명으로_다시_채운다() {
    let db = Db::memory();
    db.write(|c| save(c, input(), NOW)).unwrap();
    let mut v = as_input(&db.read(get).unwrap());
    v.school_name = "△△초등학교".into();
    v.issuer_title = "".into();
    let saved = db.write(|c| save(c, v, NOW)).unwrap();
    assert_eq!(saved.settings.issuer_title, "△△초등학교장");
    assert!(saved.issuer_filled);
}

#[test]
fn 거절된_저장은_아무것도_바꾸지_않는다() {
    let db = Db::memory();
    db.write(|c| save(c, input(), NOW)).unwrap();
    let before = db.read(get).unwrap();

    let mut v = as_input(&before);
    v.school_name = "△△초등학교".into();
    v.certificate_title = "".into();
    assert!(db.write(|c| save(c, v, "2026-10-03T00:00:00")).is_err());
    assert_eq!(db.read(get).unwrap(), before);
}

#[test]
fn 프로그램을_다시_열어도_값이_남는다() {
    let dir = tmp_dir("settings-reopen");
    let path = dir.join(DB_FILE);
    {
        let db = Db::open(&path).unwrap();
        db.write(|c| save(c, input(), NOW)).unwrap();
    } // 연결을 닫는다 = 프로그램 종료

    let db = Db::open(&path).unwrap(); // 다시 실행 (마이그레이션도 다시 돈다)
    let s = db.read(get).unwrap();
    assert_eq!(s.school_name, "○○초등학교");
    assert_eq!(s.issuer_title, "○○초등학교장");
    assert_eq!(s.manager_name, "가상담당 (인)");
    assert_eq!(s.updated_at.as_deref(), Some(NOW));
}
