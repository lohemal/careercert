use super::*;

#[test]
fn 앞뒤_공백만_정리한다() {
    let f = prepare(InstructorInput {
        name: "  김 으뜸 ".into(),
        distinguisher: " 1985년생 ".into(),
        phone: " 000-0000-0000 ".into(),
        memo: "\n메모\n".into(),
    })
    .unwrap();
    assert_eq!(f.name, "김 으뜸", "가운데 공백은 그대로");
    assert_eq!(f.distinguisher, "1985년생");
    assert_eq!(f.phone, "000-0000-0000");
    assert_eq!(f.memo, "메모");
}

#[test]
fn 이름은_비울_수_없다() {
    let e = prepare(InstructorInput {
        name: "   ".into(),
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(e.code, "INVALID_INPUT");
}

#[test]
fn 이름_말고는_비워도_된다() {
    let f = prepare(InstructorInput {
        name: "가상강사".into(),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(f.distinguisher, "");
}

#[test]
fn 너무_긴_값은_거절한다() {
    let e = prepare(InstructorInput {
        name: "가".repeat(NAME_MAX + 1),
        ..Default::default()
    })
    .unwrap_err();
    assert!(e.user_message.contains("이름"));
    assert!(prepare(InstructorInput {
        name: "가".repeat(NAME_MAX),
        ..Default::default()
    })
    .is_ok());
}
