//! 가상값만 쓴다. 주민번호 모양 줄에는 `privacy:fake` 표시를 붙인다(Git 검사 규칙).

use super::*;

#[test]
fn 하이픈과_공백을_무시하고_한_모양으로_맞춘다() {
    for s in [
        "900101-1000001",   // privacy:fake
        "9001011000001",    // privacy:fake
        " 900101 - 1000001 ", // privacy:fake
    ] {
        assert_eq!(parse(s).unwrap().as_str(), "900101-1000001"); // privacy:fake
    }
}

#[test]
fn 숫자_13자리가_아니면_거부하고_값을_말하지_않는다() {
    for s in [
        "900101-100000",    // privacy:fake (12자리)
        "900101-10000011",  // privacy:fake (14자리)
        "90O101-1000001",   // 영문 O
        "900-101-1000001",  // 하이픈 둘
    ] {
        let e = parse(s).unwrap_err();
        assert_eq!(e.code, "RRN_FORMAT", "{s}");
        assert!(!e.user_message.contains("9001"), "오류 문구에 입력값이 들어가면 안 된다");
        assert!(e.detail.is_none());
    }
    assert_eq!(parse("  ").unwrap_err().code, "RRN_REQUIRED");
}

#[test]
fn 디버그_출력에도_값이_나오지_않는다() {
    let r = parse("900101-1000001").unwrap(); // privacy:fake
    assert_eq!(format!("{r:?}"), "Rrn(******)");
}

#[test]
fn 가린_모양은_뒤_여섯_자리를_감춘다() {
    assert_eq!(parse("900101-1000001").unwrap().masked(), "900101-1******"); // privacy:fake
}

#[test]
fn 생년월일이_이상하면_알_수_있다() {
    assert!(parse("900101-1000001").unwrap().birth_date_ok()); // privacy:fake
    assert!(parse("050229-3000001").unwrap().birth_date_ok() == false); // privacy:fake (2005년은 윤년 아님)
    assert!(parse("040229-3000001").unwrap().birth_date_ok()); // privacy:fake (2004년 윤년)
    assert!(!parse("901301-1000001").unwrap().birth_date_ok()); // privacy:fake (13월)
}
