//! 복구 비밀번호 설정·변경 시험. 가상값만 쓴다.

use super::*;
use crate::db::Db;
use crate::service::history::{recovery as status, RecoveryState};
use crate::service::issuance::issuance_tests::{issue_now, world};
use crate::service::issuance::reconstruct;

const NOW: &str = "2026-10-01T09:00:00";
const PW: &str = "가상 복구 비밀번호 문장";
const PW2: &str = "새로 바꾼 가상 비밀번호";

fn pw(s: &str) -> Password {
    Password::new(s)
}

fn set_pw(db: &Db, p: &str) -> AppResult<()> {
    db.write(|c| set(c, &pw(p), &pw(p), NOW))
}

#[test]
fn 발급_전에도_설정할_수_있고_그때_키를_만든다() {
    let db = Db::memory();
    assert_eq!(db.read(status).unwrap().state, RecoveryState::NoData);
    set_pw(&db, PW).unwrap();
    assert_eq!(db.read(status).unwrap().state, RecoveryState::Ready);
    assert_eq!(db.read(|c| keys::all(c)).unwrap().len(), 1);
}

#[test]
fn 발급_뒤_설정하면_준비됨이_되고_같은_키를_감싼다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    assert_eq!(w.db.read(status).unwrap().state, RecoveryState::NotSet);
    set_pw(&w.db, PW).unwrap();
    assert_eq!(w.db.read(status).unwrap().state, RecoveryState::Ready);
    let keys = w.db.read(|c| open_all(c, &pw(PW))).unwrap();
    let local = w.db.read(|c| keys::current(c)).unwrap().unwrap();
    assert_eq!(keys[0].expose(), local.expose(), "비밀번호로 푼 키 = 이 PC 의 키");
    assert!(w.db.read(|c| reconstruct(c, id)).is_ok(), "증명서는 다시 암호화하지 않았다");
}

#[test]
fn 너무_짧거나_확인이_다르면_설정하지_않는다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    assert!(w.db.write(|c| set(c, &pw("짧은비번"), &pw("짧은비번"), NOW)).is_err());
    assert_eq!(
        w.db.write(|c| set(c, &pw(PW), &pw("다른 확인 비밀번호"), NOW)).unwrap_err().code,
        "RECOVERY_CONFIRM_MISMATCH"
    );
    assert_eq!(w.db.read(status).unwrap().state, RecoveryState::NotSet, "아무것도 저장하지 않았다");
}

#[test]
fn 비밀번호_자체는_db_어디에도_없다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    set_pw(&w.db, PW).unwrap();
    let dump: String = w
        .db
        .read(|c| {
            let mut s = String::new();
            for t in ["key_store", "audit_log", "app_meta"] {
                let mut st = c.prepare(&format!("SELECT * FROM {t}"))?;
                let n = st.column_count();
                let mut rows = st.query([])?;
                while let Some(r) = rows.next()? {
                    for i in 0..n {
                        match r.get_ref(i)? {
                            rusqlite::types::ValueRef::Text(b) | rusqlite::types::ValueRef::Blob(b) => {
                                s.push_str(&String::from_utf8_lossy(b))
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(s)
        })
        .unwrap();
    assert!(!dump.contains(PW));
    assert!(dump.contains("argon2id"), "형식 정보는 있다");
}

#[test]
fn 이미_설정했으면_다시_설정이_아니라_변경으로() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    set_pw(&w.db, PW).unwrap();
    assert_eq!(set_pw(&w.db, PW2).unwrap_err().code, "RECOVERY_ALREADY_SET");
}

#[test]
fn 변경하면_이전_비밀번호는_거절되고_새_비밀번호가_된다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    set_pw(&w.db, PW).unwrap();
    w.db.write(|c| change(c, &pw(PW), &pw(PW2), &pw(PW2), NOW)).unwrap();
    assert_eq!(w.db.read(|c| open_all(c, &pw(PW))).unwrap_err().code, "RECOVERY_PASSWORD_WRONG");
    assert!(w.db.read(|c| open_all(c, &pw(PW2))).is_ok());
    assert!(w.db.read(|c| reconstruct(c, id)).is_ok(), "데이터 키는 그대로");
}

#[test]
fn 기존_비밀번호가_틀리면_아무것도_바꾸지_않는다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    set_pw(&w.db, PW).unwrap();
    let before = w.db.read(|c| keys::all(c)).unwrap()[0].recovery_blob.clone();
    // 이 PC 의 DPAPI 로는 풀 수 있어도 기존 비밀번호 확인을 건너뛰지 않는다
    let e = w.db.write(|c| change(c, &pw("틀린 기존 비밀번호"), &pw(PW2), &pw(PW2), NOW)).unwrap_err();
    assert_eq!(e.code, "RECOVERY_PASSWORD_WRONG");
    assert_eq!(w.db.read(|c| keys::all(c)).unwrap()[0].recovery_blob, before);
    assert!(w.db.read(|c| open_all(c, &pw(PW))).is_ok());
}

#[test]
fn 설정_전에는_변경도_확인도_안_된다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    assert_eq!(w.db.write(|c| change(c, &pw(PW), &pw(PW2), &pw(PW2), NOW)).unwrap_err().code, "RECOVERY_NOT_SET");
    assert_eq!(w.db.read(|c| open_all(c, &pw(PW))).unwrap_err().code, "RECOVERY_NOT_SET");
}

#[test]
fn 변경_기록에는_비밀번호가_없다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    set_pw(&w.db, PW).unwrap();
    w.db.write(|c| change(c, &pw(PW), &pw(PW2), &pw(PW2), NOW)).unwrap();
    let log = w.db.read(|c| crate::repo::audit::all(c)).unwrap();
    let mine: Vec<&str> = log.iter().filter(|e| e.target_type == "recovery").map(|e| e.summary.as_str()).collect();
    assert_eq!(mine.len(), 2);
    for s in mine {
        assert!(!s.contains(PW) && !s.contains(PW2), "{s}");
    }
}

// ---------------- 분실 재설정 ----------------

#[test]
fn 분실_재설정은_기존_비밀번호_없이_이_pc_의_키로_새_비밀번호를_만든다() {
    let w = world();
    let id = issue_now(&w, "제2026-152호").unwrap();
    set_pw(&w.db, PW).unwrap();
    w.db.write(|c| reset(c, &pw(PW2), &pw(PW2), true, NOW)).unwrap();
    assert_eq!(w.db.read(|c| open_all(c, &pw(PW))).unwrap_err().code, "RECOVERY_PASSWORD_WRONG", "이전 비밀번호는 안 된다");
    assert!(w.db.read(|c| open_all(c, &pw(PW2))).is_ok(), "새 비밀번호로 실제로 풀린다");
    assert!(w.db.read(|c| reconstruct(c, id)).is_ok(), "데이터 키·암호문 그대로");
    let log = w.db.read(crate::repo::audit::all).unwrap();
    assert!(log.iter().any(|e| e.action == "RECOVERY_RESET" && !e.summary.contains(PW2)));
}

#[test]
fn 분실_재설정은_경고를_확인해야_하고_규칙도_같다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    set_pw(&w.db, PW).unwrap();
    assert_eq!(w.db.write(|c| reset(c, &pw(PW2), &pw(PW2), false, NOW)).unwrap_err().code, "RECOVERY_RESET_UNCONFIRMED");
    assert!(w.db.write(|c| reset(c, &pw("짧은비번"), &pw("짧은비번"), true, NOW)).is_err());
    assert_eq!(w.db.write(|c| reset(c, &pw(PW2), &pw("다른 확인 비밀번호"), true, NOW)).unwrap_err().code, "RECOVERY_CONFIRM_MISMATCH");
    assert!(w.db.read(|c| open_all(c, &pw(PW))).is_ok(), "아무것도 바뀌지 않았다");
}

#[test]
fn 이_pc_의_dpapi_로_키를_열_수_없으면_분실_재설정을_허용하지_않는다() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    set_pw(&w.db, PW).unwrap();
    // 다른 PC·계정에서 옮겨 온 자료 흉내
    w.db.write(|c| Ok(c.execute("UPDATE key_store SET dpapi_blob = X'0102030405'", [])?)).unwrap();
    let before = w.db.read(|c| keys::all(c)).unwrap()[0].recovery_blob.clone();
    assert_eq!(w.db.write(|c| reset(c, &pw(PW2), &pw(PW2), true, NOW)).unwrap_err().code, "RECOVERY_RESET_UNAVAILABLE");
    assert_eq!(w.db.read(|c| keys::all(c)).unwrap()[0].recovery_blob, before);
}

#[test]
fn 설정_전에는_분실_재설정이_아니라_설정으로() {
    let w = world();
    issue_now(&w, "제2026-152호").unwrap();
    assert_eq!(w.db.write(|c| reset(c, &pw(PW2), &pw(PW2), true, NOW)).unwrap_err().code, "RECOVERY_NOT_SET");
}
