use super::*;

fn key() -> DataKey {
    DataKey::generate("k1".into())
}

#[test]
fn aes_gcm_으로_봉인하고_연다() {
    let k = key();
    let s = seal(&k, b"cert-uuid|k1", "가상 주민번호와 주소".as_bytes()).unwrap();
    assert_eq!(s.nonce.len(), NONCE_LEN);
    assert!(!s.ciphertext.windows(6).any(|w| w == "가상".as_bytes()), "암호문에 평문이 보이면 안 된다");
    let p = open(&k, b"cert-uuid|k1", &s).unwrap();
    assert_eq!(p.as_slice(), "가상 주민번호와 주소".as_bytes());
}

#[test]
fn 같은_평문도_매번_다른_암호문이_된다() {
    let k = key();
    let a = seal(&k, b"aad", b"same").unwrap();
    let b = seal(&k, b"aad", b"same").unwrap();
    assert_ne!(a.nonce, b.nonce);
    assert_ne!(a.ciphertext, b.ciphertext);
}

#[test]
fn 암호문_nonce_aad_가_바뀌면_열리지_않는다() {
    let k = key();
    let s = seal(&k, b"cert-1", b"secret").unwrap();

    let mut c = s.clone();
    c.ciphertext[0] ^= 1;
    assert_eq!(open(&k, b"cert-1", &c).unwrap_err().code, "CRYPTO_TAMPERED");

    let mut n = s.clone();
    n.nonce[0] ^= 1;
    assert_eq!(open(&k, b"cert-1", &n).unwrap_err().code, "CRYPTO_TAMPERED");

    assert_eq!(
        open(&k, b"cert-2", &s).unwrap_err().code,
        "CRYPTO_TAMPERED",
        "다른 증명서 행에 옮겨 붙이면 열리지 않는다"
    );
    assert_eq!(open(&key(), b"cert-1", &s).unwrap_err().code, "CRYPTO_TAMPERED", "다른 키로는 안 열린다");
}

#[test]
fn 문서_지문은_키가_있어야_만들_수_있다() {
    let k = key();
    let a = doc_hash(&k, b"canonical");
    assert_eq!(a.len(), 64);
    assert_eq!(a, doc_hash(&k, b"canonical"), "같은 입력은 같은 지문");
    assert_ne!(a, doc_hash(&k, b"canonical."), "한 글자만 달라도 다른 지문");
    assert_ne!(a, doc_hash(&key(), b"canonical"), "키가 다르면 다른 지문");
    assert_ne!(a, plain_sha256(b"canonical"), "그냥 SHA-256 이 아니다");
}

#[test]
fn 키는_디버그_출력에_나오지_않는다() {
    let k = DataKey::new("k1".into(), [7u8; KEY_LEN]);
    assert_eq!(format!("{k:?}"), "DataKey(k1, ******)");
}

#[cfg(windows)]
#[test]
fn dpapi_로_감싸고_다시_푼다() {
    let secret = [42u8; KEY_LEN];
    let blob = dpapi::protect(&secret).unwrap();
    assert!(!blob.windows(KEY_LEN).any(|w| w == secret), "감싼 결과에 키가 그대로 있으면 안 된다");
    assert_eq!(dpapi::unprotect(&blob).unwrap().as_slice(), &secret);

    let mut bad = blob.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0xff;
    assert_eq!(dpapi::unprotect(&bad).unwrap_err().code, "KEY_UNAVAILABLE", "변조된 감싼 키는 풀리지 않는다");
}
