//! 경력 기간 겹침 '정상 경력으로 확인' 기록. 한 쌍에 한 행 — 다시 확인하면 지문·시각을 덮어쓴다.

use std::collections::HashSet;

use rusqlite::{params, Connection};

use crate::domain::overlap::Pair;
use crate::error::AppResult;

/// 확인해 둔 지문 전부
pub fn fingerprints(conn: &Connection) -> AppResult<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT fingerprint FROM career_overlap_acks")?;
    let set = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<HashSet<String>>>()?;
    Ok(set)
}

pub fn upsert(conn: &Connection, p: &Pair, now: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO career_overlap_acks (career_a_uuid, career_b_uuid, fingerprint, acknowledged_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (career_a_uuid, career_b_uuid)
         DO UPDATE SET fingerprint = excluded.fingerprint, acknowledged_at = excluded.acknowledged_at",
        params![p.a_uuid, p.b_uuid, p.fingerprint, now],
    )?;
    Ok(())
}
