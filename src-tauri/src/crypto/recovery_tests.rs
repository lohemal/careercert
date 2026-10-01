//! 복구 비밀번호로 감싸기 — 형식·규칙·변조. (가벼운 KDF 인자로 시험하고, 1판 기본값은 한 번만 실제로 돌린다)

use super::*;

fn pw(s: &str) -> Password {
    Password::new(s)
}

fn key() -> DataKey {
    DataKey::generate("11111111-2222-3333-4444-555555555555".into())
}

#[test]
fn 비밀번호는_10자_이상이고_확인이_같아야_한다() {
    assert_eq!(validate_new(&pw("123456789"), &pw("123456789")).unwrap_err().code, "INVALID_INPUT");
    assert!(validate_new(&pw("1234567890"), &pw("1234567890")).is_ok(), "10자는 된다");
    assert!(validate_new(&pw("가나다라마바사아자차"), &pw("가나다라마바사아자차")).is_ok(), "글자 수로 센다");
    assert_eq!(validate_new(&pw("1234567890"), &pw("1234567891")).unwrap_err().code, "RECOVERY_CONFIRM_MISMATCH");
    let long = "a".repeat(MAX_LEN);
    assert!(validate_new(&pw(&long), &pw(&long)).is_ok());
    let too_long = "a".repeat(MAX_LEN + 1);
    assert!(validate_new(&pw(&too_long), &pw(&too_long)).is_err());
    assert!(validate_new(&pw("소문자만으로도충분히긴비밀번호"), &pw("소문자만으로도충분히긴비밀번호")).is_ok(), "조합 강제 없음");
}

#[test]
fn 앞뒤_공백을_지우지_않는다() {
    let k = key();
    let blob = wrap(&k, &pw("  긴 비밀번호 문장입니다  "), KdfParams::TEST).unwrap();
    assert!(unwrap(&blob, &pw("  긴 비밀번호 문장입니다  "), &k.key_id).is_ok());
    assert_eq!(unwrap(&blob, &pw("긴 비밀번호 문장입니다"), &k.key_id).unwrap_err().code, "RECOVERY_PASSWORD_WRONG");
    // 확인 칸도 공백까지 같아야 한다
    assert!(validate_new(&pw(" 0123456789"), &pw("0123456789")).is_err());
}

#[test]
fn 올바른_비밀번호로_같은_키가_나오고_틀리면_거절한다() {
    let k = key();
    let blob = wrap(&k, &pw("올바른 복구 비밀번호"), KdfParams::TEST).unwrap();
    let back = unwrap(&blob, &pw("올바른 복구 비밀번호"), &k.key_id).unwrap();
    assert_eq!(back.expose(), k.expose());
    assert_eq!(unwrap(&blob, &pw("틀린 복구 비밀번호다"), &k.key_id).unwrap_err().code, "RECOVERY_PASSWORD_WRONG");
}

#[test]
fn blob_에는_형식_버전과_kdf_인자가_있고_비밀번호와_키는_없다() {
    let k = key();
    let blob = wrap(&k, &pw("올바른 복구 비밀번호"), KdfParams::TEST).unwrap();
    let info = describe(&blob).unwrap();
    assert_eq!(info.version, 1);
    assert_eq!(info.kdf_alg, "argon2id");
    assert_eq!(info.params, KdfParams::TEST);
    assert_eq!(info.salt_len, SALT_LEN);
    let text = String::from_utf8(blob.clone()).unwrap();
    assert!(!text.contains("올바른 복구 비밀번호"));
    assert!(!text.contains(&crate::crypto::hex(k.expose())), "키 평문(16진수)이 없다");
    let raw = k.expose();
    assert!(!blob.windows(raw.len()).any(|w| w == raw), "키 평문(바이트)이 없다");
}

#[test]
fn 감쌀_때마다_salt_와_nonce_가_다르다() {
    let k = key();
    let a = wrap(&k, &pw("올바른 복구 비밀번호"), KdfParams::TEST).unwrap();
    let b = wrap(&k, &pw("올바른 복구 비밀번호"), KdfParams::TEST).unwrap();
    assert_ne!(a, b);
}

#[test]
fn 다른_키_번호의_blob_이나_바뀐_blob_은_열리지_않는다() {
    let k = key();
    let blob = wrap(&k, &pw("올바른 복구 비밀번호"), KdfParams::TEST).unwrap();
    assert!(unwrap(&blob, &pw("올바른 복구 비밀번호"), "다른-키").is_err());
    let mut text = String::from_utf8(blob).unwrap();
    // 암호문 한 글자를 바꾼다
    let at = text.find("\"ct\":\"").unwrap() + 6;
    let c = if &text[at..at + 1] == "0" { "1" } else { "0" };
    text.replace_range(at..at + 1, c);
    assert!(unwrap(text.as_bytes(), &pw("올바른 복구 비밀번호"), &k.key_id).is_err());
}

#[test]
fn 터무니없는_kdf_인자는_읽지_않는다() {
    let k = key();
    let blob = wrap(&k, &pw("올바른 복구 비밀번호"), KdfParams::TEST).unwrap();
    let text = String::from_utf8(blob).unwrap().replace("\"m_kib\":256", "\"m_kib\":99999999");
    assert_eq!(unwrap(text.as_bytes(), &pw("올바른 복구 비밀번호"), &k.key_id).unwrap_err().code, "RECOVERY_FORMAT");
}

#[test]
fn 일판_기본값으로도_감싸고_푼다() {
    let k = key();
    let blob = wrap(&k, &pw("올바른 복구 비밀번호"), KdfParams::V1).unwrap();
    assert_eq!(describe(&blob).unwrap().params, KdfParams { m_kib: 65536, t: 3, p: 1 });
    assert!(unwrap(&blob, &pw("올바른 복구 비밀번호"), &k.key_id).is_ok());
}

#[test]
fn 비밀번호의_debug_는_값을_찍지_않는다() {
    assert_eq!(format!("{:?}", pw("올바른 복구 비밀번호")), "Password(******)");
}
