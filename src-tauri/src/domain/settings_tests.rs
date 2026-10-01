use super::*;

fn input() -> SettingsInput {
    SettingsInput {
        school_name: "○○초등학교".into(),
        certificate_title: DEFAULT_TITLE.into(),
        issuer_title: "".into(),
        department: "방과후학교".into(),
        manager_name: "가상담당".into(),
        phone: "000-0000-0000".into(),
        default_purpose: DEFAULT_PURPOSE.into(),
    }
}

#[test]
fn 기본값은_기존_양식과_같다() {
    assert_eq!(DEFAULT_TITLE, "방과후학교 개인위탁 외부강사 활동 확인서");
    assert_eq!(DEFAULT_PURPOSE, "기관제출");
}

#[test]
fn 발급자_표기는_학교명_뒤에_장을_붙여_제안한다() {
    assert_eq!(suggest_issuer("○○초등학교").as_deref(), Some("○○초등학교장"));
    assert_eq!(suggest_issuer("  ○○초등학교 ").as_deref(), Some("○○초등학교장"));
    assert_eq!(suggest_issuer("   "), None);
}

#[test]
fn 발급자_표기가_비어_있으면_학교명으로_채운다() {
    let p = prepare(input()).unwrap();
    assert_eq!(p.input.issuer_title, "○○초등학교장");
    assert!(p.issuer_filled);
}

#[test]
fn 발급자_표기에_값이_있으면_학교명과_달라도_그대로_둔다() {
    let mut v = input();
    v.school_name = "△△초등학교".into();
    v.issuer_title = "○○초등학교장 직무대리".into();
    let p = prepare(v).unwrap();
    assert_eq!(p.input.issuer_title, "○○초등학교장 직무대리");
    assert!(!p.issuer_filled);
}

#[test]
fn 학교명도_발급자도_비어_있으면_채우지_않는다() {
    let mut v = input();
    v.school_name = "".into();
    let p = prepare(v).unwrap();
    assert_eq!(p.input.issuer_title, "");
    assert!(!p.issuer_filled);
}

#[test]
fn 앞뒤_공백만_정리하고_가운데는_바꾸지_않는다() {
    let mut v = input();
    v.school_name = "  ○○  초등학교\t".into();
    v.phone = " 000 - 0000 ".into();
    v.issuer_title = " 학교장 ".into();
    let p = prepare(v).unwrap();
    assert_eq!(p.input.school_name, "○○  초등학교");
    assert_eq!(p.input.phone, "000 - 0000");
    assert_eq!(p.input.issuer_title, "학교장");
}

#[test]
fn 문서_제목과_기본_용도는_비울_수_없다() {
    let mut v = input();
    v.certificate_title = "   ".into();
    assert_eq!(prepare(v).unwrap_err().code, "INVALID_INPUT");

    let mut v = input();
    v.default_purpose = "".into();
    assert_eq!(prepare(v).unwrap_err().code, "INVALID_INPUT");
}

#[test]
fn 다른_칸은_비워_두고도_저장할_수_있다() {
    let v = SettingsInput {
        certificate_title: DEFAULT_TITLE.into(),
        default_purpose: DEFAULT_PURPOSE.into(),
        ..Default::default()
    };
    assert!(prepare(v).is_ok());
}

#[test]
fn 너무_긴_값은_거절한다() {
    let mut v = input();
    v.department = "가".repeat(MAX_LEN + 1);
    let e = prepare(v).unwrap_err();
    assert!(e.user_message.contains("담당부서"), "{}", e.user_message);

    let mut v = input();
    v.department = "가".repeat(MAX_LEN);
    assert!(prepare(v).is_ok(), "한글 {MAX_LEN}자는 받는다(바이트가 아니라 글자 수)");
}

#[test]
fn 빠진_항목을_양식_순서로_알려_준다() {
    let s = SchoolSettings {
        school_name: "○○초등학교".into(),
        certificate_title: DEFAULT_TITLE.into(),
        issuer_title: "".into(),
        department: "".into(),
        manager_name: "가상담당".into(),
        phone: " ".into(),
        default_purpose: DEFAULT_PURPOSE.into(),
        updated_at: None,
    };
    assert_eq!(
        missing(&s),
        vec![FieldKey::IssuerTitle, FieldKey::Department, FieldKey::Phone]
    );
}
