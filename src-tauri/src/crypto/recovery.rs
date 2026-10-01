//! 복구 비밀번호 — 데이터 키를 **비밀번호로 감싼 사본**(key_store.recovery_blob)과 이동용 백업 키.
//!
//! ```text
//!   복구 비밀번호 ─ Argon2id(salt 16바이트 난수, m·t·p) ─▶ KEK(32바이트)
//!   KEK ─ AES-256-GCM(nonce 12바이트 난수, AAD = "careercert/recovery/v1|{key_id}") ─▶ 데이터 키 감싸기
//! ```
//!
//! * 비밀번호에서 데이터 키를 **만들지 않는다.** 데이터 키는 Phase 6 의 난수 키 그대로이고 비밀번호는 그 키를
//!   감쌀 뿐이다 — 그래서 비밀번호를 정하거나 바꿔도 증명서 암호문은 다시 암호화하지 않는다.
//! * 비밀번호·KEK·데이터 키는 어디에도 저장하지 않고, 로그·오류에 넣지 않으며, 다 쓰면 지운다.
//! * blob 은 JSON — 형식 버전과 KDF 이름·인자·salt 가 함께 들어 있어 나중에 인자를 바꿔도 옛 blob 을 연다.
//! * 비밀번호 규칙(Phase 8 결정): 10자 이상, 1,024자 이하. 문자 종류 조합을 강제하지 않는다.
//!   **앞뒤 공백을 지우지 않는다** — 입력한 그대로가 비밀번호다.

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{hex, open, seal, unhex, DataKey, Sealed, KEY_LEN};
use crate::error::{AppError, AppResult};

pub const MIN_LEN: usize = 10;
pub const MAX_LEN: usize = 1024;
pub const SALT_LEN: usize = 16;

const BLOB_FORMAT: &str = "careercert-recovery";
const BLOB_VERSION: u32 = 1;
const KDF_ALG: &str = "argon2id";
const CIPHER: &str = "AES-256-GCM";

/// 비밀번호. 메모리에서 사라질 때 지운다. `Debug` 는 값을 찍지 않는다.
#[derive(Clone)]
pub struct Password(Zeroizing<String>);

/// 화면이 보낸 글자를 **그대로** 받는다(앞뒤 공백도 비밀번호다).
impl<'de> Deserialize<'de> for Password {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Password(Zeroizing::new(String::deserialize(d)?)))
    }
}

impl Password {
    #[cfg(test)]
    pub fn new(s: impl Into<String>) -> Self {
        Password(Zeroizing::new(s.into()))
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(******)")
    }
}

/// Argon2id 인자. blob·백업 머리에 함께 적는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// 메모리 (KiB)
    pub m_kib: u32,
    /// 반복
    pub t: u32,
    /// 병렬
    pub p: u32,
}

impl KdfParams {
    /// 1판 기본값 — 64 MiB · 3회 · 1갈래 (RFC 9106 의 두 번째 권장값)
    pub const V1: KdfParams = KdfParams { m_kib: 64 * 1024, t: 3, p: 1 };

    /// 시험용 가벼운 값 (시험이 수백 번 부른다). 형식은 같다.
    #[cfg(test)]
    pub const TEST: KdfParams = KdfParams { m_kib: 256, t: 1, p: 1 };

    /// 새로 감쌀 때 쓰는 값
    pub fn current() -> KdfParams {
        #[cfg(test)]
        {
            KdfParams::TEST
        }
        #[cfg(not(test))]
        {
            KdfParams::V1
        }
    }

    /// 파일에서 읽은 인자가 터무니없지 않은가 (변조된 파일이 메모리를 수 GB 쓰게 하지 않도록)
    fn sane(self) -> bool {
        (8..=1024 * 1024).contains(&self.m_kib) && (1..=16).contains(&self.t) && (1..=8).contains(&self.p)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kdf {
    pub alg: String,
    /// Argon2 버전 (0x13 = 19)
    pub version: u32,
    #[serde(flatten)]
    pub params: KdfParams,
    /// 16진수
    pub salt: String,
}

impl Kdf {
    pub fn fresh(params: KdfParams) -> Kdf {
        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        Kdf { alg: KDF_ALG.into(), version: 0x13, params, salt: hex(&salt) }
    }

    /// 비밀번호 → 32바이트 키
    pub fn derive(&self, password: &Password) -> AppResult<Zeroizing<[u8; KEY_LEN]>> {
        if self.alg != KDF_ALG || self.version != 0x13 || !self.params.sane() {
            return Err(bad_format("kdf"));
        }
        let salt = unhex(&self.salt).ok_or_else(|| bad_format("salt"))?;
        if salt.len() < 8 {
            return Err(bad_format("salt"));
        }
        let params = Params::new(self.params.m_kib, self.params.t, self.params.p, Some(KEY_LEN))
            .map_err(|_| bad_format("params"))?;
        let mut out = Zeroizing::new([0u8; KEY_LEN]);
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(password.expose().as_bytes(), &salt, &mut out[..])
            .map_err(|_| bad_format("argon2"))?;
        Ok(out)
    }
}

fn bad_format(what: &str) -> AppError {
    AppError::new("RECOVERY_FORMAT", "복구 정보의 형식을 읽을 수 없습니다.").detail(what.to_string())
}

pub fn wrong_password() -> AppError {
    AppError::new("RECOVERY_PASSWORD_WRONG", "복구 비밀번호가 맞지 않습니다.")
}

/// 새 비밀번호 규칙 검사. 앞뒤 공백도 비밀번호의 일부다(지우지 않는다).
pub fn validate_new(password: &Password, confirm: &Password) -> AppResult<()> {
    let n = password.expose().chars().count();
    if n < MIN_LEN {
        return Err(AppError::invalid(format!("복구 비밀번호는 {MIN_LEN}자 이상이어야 합니다.")));
    }
    if n > MAX_LEN {
        return Err(AppError::invalid(format!("복구 비밀번호는 {MAX_LEN}자 이하로 정해 주세요.")));
    }
    if password.expose() != confirm.expose() {
        return Err(AppError::new("RECOVERY_CONFIRM_MISMATCH", "비밀번호 확인이 일치하지 않습니다. 저장하지 않았습니다."));
    }
    Ok(())
}

/// recovery_blob 1판 (JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlobV1 {
    format: String,
    v: u32,
    key_id: String,
    kdf: Kdf,
    cipher: String,
    nonce: String,
    ct: String,
}

fn aad(key_id: &str) -> Vec<u8> {
    format!("careercert/recovery/v1|{key_id}").into_bytes()
}

/// 데이터 키를 비밀번호로 감싼다 — 새 salt·nonce.
pub fn wrap(key: &DataKey, password: &Password, params: KdfParams) -> AppResult<Vec<u8>> {
    let kdf = Kdf::fresh(params);
    let kek = DataKey::new("recovery-kek".into(), *kdf.derive(password)?);
    let sealed = seal(&kek, &aad(&key.key_id), key.expose())?;
    let blob = BlobV1 {
        format: BLOB_FORMAT.into(),
        v: BLOB_VERSION,
        key_id: key.key_id.clone(),
        kdf,
        cipher: CIPHER.into(),
        nonce: hex(&sealed.nonce),
        ct: hex(&sealed.ciphertext),
    };
    Ok(serde_json::to_vec(&blob)?)
}

/// 감싼 사본을 비밀번호로 푼다. 비밀번호가 틀리면 `RECOVERY_PASSWORD_WRONG`.
pub fn unwrap(blob: &[u8], password: &Password, key_id: &str) -> AppResult<DataKey> {
    let b: BlobV1 = serde_json::from_slice(blob).map_err(|_| bad_format("json"))?;
    if b.format != BLOB_FORMAT || b.v != BLOB_VERSION || b.cipher != CIPHER {
        return Err(bad_format("version"));
    }
    if b.key_id != key_id {
        return Err(bad_format("key_id"));
    }
    let kek = DataKey::new("recovery-kek".into(), *b.kdf.derive(password)?);
    let sealed = Sealed {
        nonce: unhex(&b.nonce).ok_or_else(|| bad_format("nonce"))?,
        ciphertext: unhex(&b.ct).ok_or_else(|| bad_format("ct"))?,
    };
    let plain = open(&kek, &aad(key_id), &sealed).map_err(|_| wrong_password())?;
    if plain.len() != KEY_LEN {
        return Err(bad_format("length"));
    }
    let mut k = [0u8; KEY_LEN];
    k.copy_from_slice(&plain);
    Ok(DataKey::new(key_id.to_string(), k))
}

/// blob 의 형식 정보 (시험용 — 비밀 값은 없다)
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobInfo {
    pub version: u32,
    pub kdf_alg: String,
    pub params: KdfParams,
    pub salt_len: usize,
}

#[cfg(test)]
pub fn describe(blob: &[u8]) -> AppResult<BlobInfo> {
    let b: BlobV1 = serde_json::from_slice(blob).map_err(|_| bad_format("json"))?;
    Ok(BlobInfo {
        version: b.v,
        kdf_alg: b.kdf.alg.clone(),
        params: b.kdf.params,
        salt_len: unhex(&b.kdf.salt).map(|s| s.len()).unwrap_or(0),
    })
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod recovery_tests;
