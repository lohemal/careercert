//! 이동용 백업 패키지(`.careercert-backup`) 만들기·열기와 복원 준비.
//!
//! 자동 백업(앱 자료 폴더 `backups/*.db`)과 다르다 — 그것은 이 PC 안에서 되돌리기용이고 비밀번호를 묻지 않는다.
//! 이동용 백업은 **파일 전체를 복구 비밀번호로 암호화**한다. 주민번호·주소는 원래 암호문이지만 강사 이름·연락처·
//! 경력·학교 정보도 업무 자료이므로 파일을 열거나 SQLite 도구로 읽어도 보이지 않아야 한다.
//!
//! ```text
//!   파일 = "CCBACKUP"(8) | 머리 길이 u32 LE(4) | 머리 JSON | nonce(12) | 암호문(+태그 16)
//!   머리 = 형식 버전 · app_id · 자료 구조 버전 · 만든 시각 · 프로그램 버전 · KDF(argon2id, 인자, salt) · 암호 방식
//!   키   = Argon2id(복구 비밀번호, 머리의 salt)            ← 패키지마다 새 salt
//!   암호 = AES-256-GCM(키, nonce, 본문 = SQLite DB 바이트, AAD = 앞의 8+4+머리) ← 머리를 바꿔도 열리지 않는다
//! ```
//!
//! **평문 DB 를 디스크에 쓰지 않는다.** 만들 때는 SQLite 백업 API 로 메모리 DB 에 뜬 뒤 `serialize`,
//! 열 때는 복호화한 바이트를 메모리 DB 로 `deserialize` 해서 검사·마이그레이션·키 재포장·발급본 검증을
//! 모두 메모리에서 한다. 디스크에 쓰는 것은 검증이 끝난 복원 대기 파일(`*.restore-pending`, 다음 실행에서
//! 실제 자료가 된다)뿐이다.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, DatabaseName};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto::recovery::{Kdf, KdfParams, Password};
use crate::crypto::{self, keys, DataKey, Sealed};
use crate::db::{migrate, APP_ID};
use crate::error::{AppError, AppResult};
use crate::service::{issuance, recovery};

pub const EXTENSION: &str = "careercert-backup";
const MAGIC: &[u8; 8] = b"CCBACKUP";
const FORMAT: &str = "careercert-backup";
const FORMAT_VERSION: u32 = 1;
const MAX_HEADER: usize = 64 * 1024;

/// 패키지 머리 (암호화하지 않는다 — 개인정보·업무 자료 없음)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Header {
    pub format: String,
    pub format_version: u32,
    pub app_id: String,
    pub app_version: String,
    pub schema_version: i32,
    /// 만든 시각 `YYYY-MM-DDTHH:MM:SS`
    pub created_at: String,
    pub cipher: String,
    pub kdf: Kdf,
    /// 본문(평문 DB) 크기
    pub payload_len: u64,
}

impl PartialEq for Kdf {
    fn eq(&self, o: &Self) -> bool {
        self.alg == o.alg && self.version == o.version && self.params == o.params && self.salt == o.salt
    }
}
impl Eq for Kdf {}

/// `방과후강사경력관리_백업_20261001.careercert-backup` — 개인정보·학교명 없음
pub fn default_file_name(now: &str) -> String {
    let day: String = now.chars().take(10).filter(|c| c.is_ascii_digit()).collect();
    format!("방과후강사경력관리_백업_{day}.{EXTENSION}")
}

fn bad_package(detail: &str) -> AppError {
    AppError::new("BACKUP_INVALID", "이 프로그램의 이동용 백업 파일이 아니거나 파일이 손상되었습니다.").detail(detail.to_string())
}

fn wrong_password_or_tampered() -> AppError {
    AppError::new(
        "BACKUP_UNLOCK_FAILED",
        "복구 비밀번호가 맞지 않거나 백업 파일이 손상·변조되었습니다. 복원하지 않았습니다.",
    )
}

/// 열린 자료를 메모리 DB 로 뜬다 (SQLite 백업 API — WAL 내용 포함, 디스크에 쓰지 않음)
pub fn snapshot_to_memory(conn: &Connection) -> AppResult<Connection> {
    let mut mem = Connection::open_in_memory()?;
    {
        let b = rusqlite::backup::Backup::new(conn, &mut mem)?;
        b.run_to_completion(500, std::time::Duration::ZERO, None)?;
    }
    Ok(mem)
}

fn integrity_ok(conn: &Connection) -> AppResult<()> {
    let v: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if v == "ok" {
        Ok(())
    } else {
        Err(AppError::new("BACKUP_INTEGRITY", "백업 안의 자료가 손상되었습니다(무결성 검사 실패).").detail(v))
    }
}

fn app_id_ok(conn: &Connection) -> AppResult<()> {
    let id: Option<String> = conn.query_row("SELECT value FROM app_meta WHERE key = 'app_id'", [], |r| r.get(0)).ok();
    if id.as_deref() == Some(APP_ID) {
        Ok(())
    } else {
        Err(AppError::new("BACKUP_OTHER_APP", "이 프로그램의 자료가 아닙니다."))
    }
}

/// 이동용 백업 패키지 바이트를 만든다. 비밀번호가 지금 recovery_blob 을 실제로 풀어야 한다.
pub fn build(conn: &Connection, password: &Password, now: &str, app_version: &str) -> AppResult<Vec<u8>> {
    // 1) 복구 설정과 비밀번호 확인 — 틀리면 아무것도 만들지 않는다
    recovery::open_all(conn, password)?;
    // 2) 메모리로 뜨고 검사
    let mem = snapshot_to_memory(conn)?;
    integrity_ok(&mem)?;
    app_id_ok(&mem)?;
    let schema_version = migrate::current_version(&mem)?;
    let plain: Zeroizing<Vec<u8>> = Zeroizing::new(mem.serialize(DatabaseName::Main)?.to_vec());
    drop(mem);
    // 3) 암호화
    seal_bytes(&plain, schema_version, password, now, app_version)
}

/// DB 바이트를 패키지로 봉인한다 (검사는 부르는 쪽이 한다 — 시험이 일부러 망가진 자료를 넣어 보는 데도 쓴다)
pub(crate) fn seal_bytes(plain: &[u8], schema_version: i32, password: &Password, now: &str, app_version: &str) -> AppResult<Vec<u8>> {
    let kdf = Kdf::fresh(KdfParams::current());
    let key = DataKey::new("backup-package".into(), *kdf.derive(password)?);
    let header = Header {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        app_id: APP_ID.into(),
        app_version: app_version.into(),
        schema_version,
        created_at: now.into(),
        cipher: "AES-256-GCM".into(),
        kdf,
        payload_len: plain.len() as u64,
    };
    let head = serde_json::to_vec(&header)?;
    let mut prefix = Vec::with_capacity(12 + head.len());
    prefix.extend_from_slice(MAGIC);
    prefix.extend_from_slice(&(head.len() as u32).to_le_bytes());
    prefix.extend_from_slice(&head);
    let sealed = crypto::seal(&key, &prefix, plain)?;
    let mut out = prefix;
    out.extend_from_slice(&sealed.nonce);
    out.extend_from_slice(&sealed.ciphertext);
    Ok(out)
}

/// 머리만 읽는다 (비밀번호 없이 — 형식·버전 검사와 미리보기 첫 단계)
pub fn read_header(bytes: &[u8]) -> AppResult<Header> {
    Ok(split(bytes)?.0)
}

fn split(bytes: &[u8]) -> AppResult<(Header, &[u8], &[u8], &[u8])> {
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return Err(bad_package("magic"));
    }
    let n = u32::from_le_bytes(bytes[8..12].try_into().expect("4바이트")) as usize;
    if n == 0 || n > MAX_HEADER || bytes.len() < 12 + n + crypto::NONCE_LEN + 16 {
        return Err(bad_package("length"));
    }
    let header: Header = serde_json::from_slice(&bytes[12..12 + n]).map_err(|_| bad_package("header"))?;
    if header.format != FORMAT || header.app_id != APP_ID {
        return Err(AppError::new("BACKUP_OTHER_APP", "이 프로그램의 이동용 백업 파일이 아닙니다."));
    }
    if header.format_version != FORMAT_VERSION {
        return Err(AppError::new(
            "BACKUP_FORMAT_VERSION",
            "이 프로그램이 읽을 수 없는 백업 형식입니다. 프로그램을 최신 버전으로 업데이트해 주세요.",
        ));
    }
    if header.schema_version > migrate::latest_version() {
        return Err(AppError::new(
            "SCHEMA_TOO_NEW",
            format!(
                "더 최신 버전의 프로그램에서 만든 백업입니다(자료 구조 v{}). 프로그램을 먼저 업데이트해 주세요.",
                header.schema_version
            ),
        ));
    }
    if header.schema_version < 1 {
        return Err(bad_package("schema"));
    }
    let prefix = &bytes[..12 + n];
    let nonce = &bytes[12 + n..12 + n + crypto::NONCE_LEN];
    let ct = &bytes[12 + n + crypto::NONCE_LEN..];
    Ok((header, prefix, nonce, ct))
}

/// 패키지를 풀어 **메모리 DB** 로 연다. 무결성·app_id·구조 버전까지 확인한다.
pub fn open(bytes: &[u8], password: &Password) -> AppResult<(Header, Connection)> {
    let (header, prefix, nonce, ct) = split(bytes)?;
    let key = DataKey::new("backup-package".into(), *header.kdf.derive(password)?);
    let plain = crypto::open(&key, prefix, &Sealed { nonce: nonce.to_vec(), ciphertext: ct.to_vec() })
        .map_err(|_| wrong_password_or_tampered())?;
    if plain.len() as u64 != header.payload_len {
        return Err(bad_package("payload length"));
    }
    let mut mem = Connection::open_in_memory()?;
    deserialize(&mut mem, &plain)?;
    drop(plain);
    integrity_ok(&mem)?;
    app_id_ok(&mem)?;
    let v = migrate::current_version(&mem)?;
    if v != header.schema_version {
        return Err(bad_package("schema mismatch"));
    }
    Ok((header, mem))
}

/// 바이트를 메모리 DB 로 (SQLite 가 가진 메모리로 옮긴 뒤 넘긴다)
fn deserialize(mem: &mut Connection, bytes: &[u8]) -> AppResult<()> {
    use rusqlite::ffi;
    use std::ptr::NonNull;
    if bytes.is_empty() {
        return Err(bad_package("empty"));
    }
    // SAFETY: sqlite3_malloc64 로 받은 버퍼에 그대로 복사하고, 소유권을 OwnedData 로 넘긴다
    // (deserialize 가 SQLITE_DESERIALIZE_FREEONCLOSE 로 그 버퍼를 맡는다).
    let data = unsafe {
        let ptr = ffi::sqlite3_malloc64(bytes.len() as u64) as *mut u8;
        let ptr = NonNull::new(ptr).ok_or_else(|| AppError::internal("메모리를 잡지 못했습니다."))?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.as_ptr(), bytes.len());
        // 원래 자료는 WAL 모드라 파일 머리 18·19번째 바이트가 2(WAL)다. 메모리 DB 는 -wal 파일을 열 수 없어
        // 쓰기가 실패하므로 1(일반 저널)로 돌린다 — 내용은 바뀌지 않는다(SQLite 파일 형식 문서의 머리 정의).
        if bytes.len() > 19 {
            *ptr.as_ptr().add(18) = 1;
            *ptr.as_ptr().add(19) = 1;
        }
        rusqlite::serialize::OwnedData::from_raw_nonnull(ptr, bytes.len())
    };
    mem.deserialize(DatabaseName::Main, data, false)
        .map_err(|_| bad_package("sqlite"))?;
    Ok(())
}

// ---------------------------------------------------------------
// 복원 준비
// ---------------------------------------------------------------

/// 자료 건수 (미리보기·검증용)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub instructors: i64,
    pub careers: i64,
    pub certificates_issued: i64,
    pub certificates_voided: i64,
    pub imports: i64,
}

pub fn counts(conn: &Connection) -> AppResult<Counts> {
    let one = |sql: &str| -> AppResult<i64> { Ok(conn.query_row(sql, [], |r| r.get(0))?) };
    let has = |t: &str| -> AppResult<bool> {
        Ok(conn.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [t], |r| r.get::<_, i64>(0))? > 0)
    };
    let (issued, voided) = if has("certificates")? {
        (
            one("SELECT count(*) FROM certificates WHERE status = 'ISSUED'")?,
            one("SELECT count(*) FROM certificates WHERE status = 'VOIDED'")?,
        )
    } else {
        (0, 0)
    };
    Ok(Counts {
        instructors: if has("instructors")? { one("SELECT count(*) FROM instructors")? } else { 0 },
        careers: if has("careers")? { one("SELECT count(*) FROM careers")? } else { 0 },
        certificates_issued: issued,
        certificates_voided: voided,
        imports: if has("imports")? { one("SELECT count(*) FROM imports")? } else { 0 },
    })
}

/// 모든 발급본을 다시 만들어 본다 — 복호화·지문 검증. 하나라도 안 되면 실패.
pub fn verify_certificates(conn: &Connection) -> AppResult<usize> {
    verify_certificates_with(conn, &mut |_, _| {})
}

/// 진행을 알리며 검증한다 (`progress(done, total)` — 화면이 멈춘 것처럼 보이지 않게)
pub fn verify_certificates_with(conn: &Connection, progress: &mut dyn FnMut(usize, usize)) -> AppResult<usize> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM certificates ORDER BY id")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let total = ids.len();
    let mut cache = issuance::KeyCache::default();
    progress(0, total);
    for (n, id) in ids.iter().enumerate() {
        if n % 200 == 0 {
            progress(n, total);
        }
        issuance::reconstruct_with(conn, *id, &mut cache).map_err(|e| {
            AppError::new(
                "RESTORE_VERIFY_FAILED",
                "백업 안의 발급 기록을 확인하지 못했습니다(복호화 또는 문서 지문 검증 실패). 복원하지 않았습니다.",
            )
            .detail(format!("certificate {id}: {}", e.code))
        })?;
    }
    progress(total, total);
    Ok(total)
}

/// 복원할 준비가 끝난 자료 (메모리 DB — 마이그레이션·이 PC DPAPI 재포장·발급본 검증까지 마침)
pub struct Prepared {
    pub header: Header,
    /// 백업에 들어 있던 그대로의 건수
    pub counts: Counts,
    /// 백업의 자료 구조 버전 → 지금 프로그램 버전으로 올렸는가
    pub migrated_from: i32,
    pub verified_certificates: usize,
    /// 이 PC 의 DPAPI 로 원래 사본을 풀 수 있었는가 (false = 다른 PC·계정 — 비밀번호 사본으로 풀어 다시 감쌈)
    pub same_windows_user: bool,
    pub conn: Connection,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared").field("header", &self.header).field("counts", &self.counts).finish_non_exhaustive()
    }
}

/// 패키지 → 메모리 DB → 검사 → 마이그레이션 → 데이터 키 복구(비밀번호) → 이 PC DPAPI 로 재포장 → 모든 발급본 검증.
/// 지금 자료는 건드리지 않는다.
#[cfg(test)]
pub fn prepare(bytes: &[u8], password: &Password) -> AppResult<Prepared> {
    prepare_with(bytes, password, &mut |_, _| {})
}

/// `prepare` 와 같다. 발급본 검증 진행을 알린다.
pub fn prepare_with(bytes: &[u8], password: &Password, progress: &mut dyn FnMut(usize, usize)) -> AppResult<Prepared> {
    let (header, mut mem) = open(bytes, password)?;
    let before = counts(&mem)?;
    let migrated_from = migrate::current_version(&mem)?;
    mem.pragma_update(None, "foreign_keys", "ON")?;
    migrate::run(&mut mem, Path::new(":memory:"))?; // 메모리라 마이그레이션 전 백업은 뜨지 않는다
    integrity_ok(&mem)?;

    // 데이터 키 — 비밀번호 사본으로 푼다(같은 PC 면 DPAPI 사본과 같은 키인지도 맞춰 본다)
    let rows = keys::all(&mem)?;
    let same_windows_user = !rows.is_empty() && rows.iter().all(|r| keys::open_dpapi(r).is_ok());
    let opened = if rows.is_empty() { Vec::new() } else { recovery::open_all(&mem, password)? };
    for key in &opened {
        keys::rewrap_dpapi(&mem, key)?; // 이 PC·이 Windows 사용자로 다시 감싼다(암호문은 그대로)
    }
    let verified_certificates = verify_certificates_with(&mem, progress)?;
    if counts(&mem)? != before {
        return Err(AppError::internal("마이그레이션 뒤 건수가 달라졌습니다."));
    }
    Ok(Prepared { header, counts: before, migrated_from, verified_certificates, same_windows_user, conn: mem })
}

// ---------------------------------------------------------------
// 파일 쓰기 (.part → 이름 바꾸기)
// ---------------------------------------------------------------

/// `path` 에 바이트를 쓴다. `.part` 로 쓰고 디스크에 내린 뒤 이름을 바꾼다. 실패하면 `.part` 를 지운다.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    use std::io::Write;
    let part = part_path(path);
    let result = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&part)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&part, path)
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(AppError::new("BACKUP_WRITE_FAILED", "백업 파일을 쓰지 못했습니다.").detail(e.kind().to_string()));
    }
    Ok(())
}

pub fn part_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

#[cfg(test)]
#[path = "portable_tests.rs"]
pub(crate) mod portable_tests;
