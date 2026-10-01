//! 주민번호·주소 암호화와 문서 지문 (설계안 §8·§15.1).
//!
//! ```text
//!   데이터 키(DEK, 32바이트 난수)
//!     ├─ Windows DPAPI(현재 사용자)로 감싼 사본 ─▶ key_store.dpapi_blob   (같은 PC: 비밀번호 없이 풀림)
//!     └─ 복구 비밀번호로 감싼 사본               ─▶ key_store.recovery_blob (Phase 8)
//!
//!   주민번호·주소 ─ AES-256-GCM(DEK, 증명서마다 새 nonce 12바이트, AAD=증명서 uuid·키 번호) ─▶ certificates
//!   문서 지문      ─ HMAC-SHA256(DEK 에서 나눈 지문 키, 정규화한 문서) ─▶ certificates.doc_hash
//! ```
//!
//! **문서 지문을 그냥 SHA-256 으로 하지 않는 까닭**: 지문의 입력에 주민번호가 들어간다. 주민번호는
//! 경우의 수가 작아(생년월일 × 뒷자리) DB 를 손에 넣은 사람이 지문만 보고 하나씩 맞춰 볼 수 있다.
//! 키를 모르면 맞춰 볼 수 없게 HMAC 으로 만든다.
//!
//! 지키는 것: 키·평문을 로그·오류·파일에 남기지 않는다. 다 쓴 평문 버퍼는 지운다(zeroize).

pub mod dpapi;
pub mod keys;
pub mod recovery;

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::AppError;

pub const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;

/// 풀린 데이터 키. 메모리에서 사라질 때 0 으로 지운다. `Debug` 는 값을 찍지 않는다.
#[derive(Clone)]
pub struct DataKey {
    pub key_id: String,
    bytes: Zeroizing<[u8; KEY_LEN]>,
}

impl std::fmt::Debug for DataKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DataKey({}, ******)", self.key_id)
    }
}

impl DataKey {
    pub fn new(key_id: String, bytes: [u8; KEY_LEN]) -> Self {
        DataKey { key_id, bytes: Zeroizing::new(bytes) }
    }

    /// 새 난수 키
    pub fn generate(key_id: String) -> Self {
        let k = Aes256Gcm::generate_key(OsRng);
        let mut b = [0u8; KEY_LEN];
        b.copy_from_slice(&k);
        DataKey::new(key_id, b)
    }

    pub(crate) fn expose(&self) -> &[u8; KEY_LEN] {
        &self.bytes
    }

    /// 문서 지문 키 = HMAC(DEK, "careercert/doc-hash/v1") — 암호화 키를 그대로 두 곳에 쓰지 않는다.
    fn hash_key(&self) -> Zeroizing<Vec<u8>> {
        let mut m = <Hmac<Sha256> as Mac>::new_from_slice(&self.bytes[..]).expect("HMAC 키 길이는 무엇이든 된다");
        m.update(b"careercert/doc-hash/v1");
        Zeroizing::new(m.finalize().into_bytes().to_vec())
    }
}

fn crypto_err(detail: &str) -> AppError {
    AppError::new("CRYPTO_FAILED", "암호화된 자료를 처리하지 못했습니다.").detail(detail.to_string())
}

/// 봉인한 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    pub nonce: Vec<u8>,
    /// 암호문 + 인증 태그
    pub ciphertext: Vec<u8>,
}

/// AES-256-GCM 으로 봉인한다. nonce 는 매번 새 난수.
pub fn seal(key: &DataKey, aad: &[u8], plaintext: &[u8]) -> Result<Sealed, AppError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.expose()));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, Payload { msg: plaintext, aad })
        .map_err(|_| crypto_err("encrypt"))?;
    Ok(Sealed { nonce: nonce.to_vec(), ciphertext })
}

/// 봉인을 연다. 암호문·nonce·AAD 가 하나라도 바뀌었으면 실패한다(변조 감지).
pub fn open(key: &DataKey, aad: &[u8], sealed: &Sealed) -> Result<Zeroizing<Vec<u8>>, AppError> {
    if sealed.nonce.len() != NONCE_LEN {
        return Err(crypto_err("nonce length"));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.expose()));
    cipher
        .decrypt(Nonce::from_slice(&sealed.nonce), Payload { msg: &sealed.ciphertext, aad })
        .map(Zeroizing::new)
        .map_err(|_| {
            AppError::new(
                "CRYPTO_TAMPERED",
                "암호화된 자료가 손상되었거나 바뀌었습니다. 이 증명서는 출력할 수 없습니다.",
            )
        })
}

/// 문서 지문 — HMAC-SHA256(지문 키, canonical) 의 16진수 64자.
pub fn doc_hash(key: &DataKey, canonical: &[u8]) -> String {
    let hk = key.hash_key();
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(&hk).expect("HMAC 키 길이는 무엇이든 된다");
    m.update(canonical);
    hex(&m.finalize().into_bytes())
}

/// 개인정보가 **없는** 문서 표현의 SHA-256 — 내용 확인과 발급 확정 사이에 자료가 바뀌었는지 보는 표.
pub fn plain_sha256(data: &[u8]) -> String {
    use sha2::Digest;
    hex(&Sha256::digest(data))
}

pub(crate) fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 16진수 → 바이트. 형식이 틀리면 None.
pub(crate) fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

#[cfg(test)]
#[path = "crypto_tests.rs"]
mod crypto_tests;
