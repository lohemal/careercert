//! 오발급 폐기 전용 SQL — **이 모듈 밖에서는 발급 기록을 지우지 않는다.**
//!
//! 발급 기록·경력 줄·출력 이력·로고 보관본은 DB 트리거가 지우기를 막는다(006·008). 009 가 단 예외는 하나:
//! 폐기 허가 표(`certificate_discard_gate`)에 그 발급 번호가 있는 동안만. 여기서 허가를 넣고 → 딸린 것을 지우고 →
//! 허가를 지운다. 부르는 쪽(`service::issuance::discard`)이 한 트랜잭션(`Db::write`) 안에서 부른다 —
//! 중간에 실패하면 전부 되돌려지고 허가도 남지 않는다.

use rusqlite::{params, Connection};

use crate::error::AppResult;

/// 폐기한 것 (감사 기록용 건수)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discarded {
    pub uuid: String,
    pub items: usize,
    pub outputs: usize,
    /// copied_from 으로 이 발급 기록을 가리키던 다른 발급 기록 (그 칸만 비웠다)
    pub unlinked: usize,
    /// 다른 발급 기록이 쓰지 않아 함께 지운 로고 보관본
    pub logos: usize,
}

/// 발급 기록 하나와 거기 딸린 것을 지운다. ISSUED 가 아니면 허가 넣기에서 DB 가 거절한다.
pub(crate) fn discard_certificate(conn: &Connection, id: i64) -> AppResult<Discarded> {
    let (uuid, logo): (String, Option<String>) = conn.query_row(
        "SELECT uuid, logo_sha256 FROM certificates WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    conn.execute("INSERT INTO certificate_discard_gate (certificate_id) VALUES (?1)", [id])?;
    let unlinked = conn.execute(
        "UPDATE certificates SET copied_from_certificate_id = NULL WHERE copied_from_certificate_id = ?1",
        [id],
    )?;
    let outputs = conn.execute("DELETE FROM certificate_outputs WHERE certificate_id = ?1", [id])?;
    let items = conn.execute("DELETE FROM certificate_items WHERE certificate_id = ?1", [id])?;
    conn.execute("DELETE FROM certificates WHERE id = ?1", [id])?;
    let logos = match logo {
        Some(sha) => conn.execute(
            "DELETE FROM certificate_logos WHERE sha256 = ?1
               AND NOT EXISTS (SELECT 1 FROM certificates WHERE logo_sha256 = ?1)",
            params![sha],
        )?,
        None => 0,
    };
    conn.execute("DELETE FROM certificate_discard_gate WHERE certificate_id = ?1", [id])?;
    Ok(Discarded { uuid, items, outputs, unlinked, logos })
}

/// 남은 허가가 있는가 (시험용 — 정상이면 언제나 없다)
#[cfg(test)]
pub fn gate_open(conn: &Connection) -> AppResult<bool> {
    Ok(conn.query_row("SELECT EXISTS (SELECT 1 FROM certificate_discard_gate)", [], |r| r.get(0))?)
}
