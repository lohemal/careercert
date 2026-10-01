//! v0.1.2 학교 로고 시험 — 등록·발급 스냅샷·불변성·무결성·1판 호환·백업. 시험용 로고는 코드로 그린다.

use image::ImageFormat;

use super::*;
use crate::db::testutil::tmp_dir;
use crate::db::{backup, Db, DB_FILE};
use crate::domain::certificate::{self as cert, TEMPLATE_VERSION};
use crate::domain::logo::logo_tests::fake_logo;
use crate::render::{self, Mode};
use crate::service::certificate as drafting;
use crate::service::issuance::issuance_tests::{issue_now, request, sql, world, world_in, World, NOW};
use crate::service::issuance::{self, for_output, reconstruct, store};
use crate::service::portable::portable_tests::{package, pw, PW};
use crate::service::portable::prepare;
use crate::service::{recovery, restore};

fn logo_a() -> Vec<u8> {
    fake_logo(400, 400, ImageFormat::Png)
}
fn logo_b() -> Vec<u8> {
    fake_logo(600, 240, ImageFormat::Jpeg)
}

fn set_logo(w: &World, bytes: &[u8]) -> Logo {
    w.db.write(|c| set(c, bytes, NOW)).unwrap()
}

fn cert_logo(w: &World, id: i64) -> Option<String> {
    w.db.read(|c| reconstruct(c, id)).unwrap().doc.logo.map(|l| l.sha256)
}

// ---------------- 설정 ----------------

#[test]
fn png_와_jpg_를_등록하고_지울_수_있다() {
    let w = world();
    let a = set_logo(&w, &logo_a());
    assert_eq!(w.db.read(crate::repo::logo::current).unwrap().map(|l| l.sha256), Some(a.sha256.clone()));
    let b = set_logo(&w, &logo_b());
    assert_eq!((b.width, b.height), (600, 240));
    assert_eq!(w.db.read(crate::repo::logo::current).unwrap().unwrap().sha256, b.sha256, "바꾸면 새 로고");
    assert!(w.db.write(|c| remove(c, NOW)).unwrap());
    assert!(w.db.read(crate::repo::logo::current).unwrap().is_none());
    assert!(!w.db.write(|c| remove(c, NOW)).unwrap(), "없으면 아무것도 안 함");
}

#[test]
fn 깨지거나_지원하지_않거나_너무_큰_파일은_거절하고_바뀌지_않는다() {
    let w = world();
    let a = set_logo(&w, &logo_a());
    let mut broken = logo_b();
    broken.truncate(broken.len() / 3);
    for bad in [broken, b"GIF89a....".to_vec(), b"BM......".to_vec(), vec![0u8; crate::domain::logo::MAX_FILE_BYTES + 1]] {
        assert_eq!(w.db.write(|c| set(c, &bad, NOW)).unwrap_err().code, "LOGO_INVALID");
    }
    assert_eq!(w.db.read(crate::repo::logo::current).unwrap().unwrap().sha256, a.sha256, "거절되면 그대로");
}

#[test]
fn 원본_경로는_어디에도_남지_않는다() {
    let w = world();
    set_logo(&w, &logo_a());
    let cols: Vec<String> = w
        .db
        .read(|c| Ok(c.prepare("SELECT name FROM pragma_table_info('school_logo')")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?))
        .unwrap();
    assert_eq!(cols, vec!["id", "mime", "data", "sha256", "width", "height", "updated_at"], "경로 칸이 없다");
    let log = w.db.read(crate::repo::audit::all).unwrap();
    let s = &log.iter().find(|e| e.action == "SCHOOL_LOGO").unwrap().summary;
    assert_eq!(s, "학교 로고 등록 · 400×400");
}

#[test]
fn 로고_없이도_설정과_발급이_된다() {
    let w = world();
    let id = issue_now(&w, "제2026-001호").unwrap();
    let r = w.db.read(|c| reconstruct(c, id)).unwrap();
    assert_eq!(r.doc.template_version, 2);
    assert!(r.doc.logo.is_none());
    let h = render::render(&r.doc, Mode::PreviewDraft).unwrap();
    assert!(!h.contains("background-image"), "로고 없으면 배경 없음");
    let body = &h[h.find("<body>").unwrap()..];
    assert!(!body.contains("로고"), "'로고 없음' 같은 글자도 없다");
}

// ---------------- 2판 ----------------

#[test]
fn 새_발급은_2판이고_로고가_스냅샷에_들어간다() {
    let w = world();
    let a = set_logo(&w, &logo_a());
    let id = issue_now(&w, "제2026-001호").unwrap();
    assert_eq!(TEMPLATE_VERSION, 2);
    let r = w.db.read(|c| reconstruct(c, id)).unwrap();
    assert_eq!(r.row.template_version, 2);
    assert_eq!(r.row.logo_sha256.as_deref(), Some(a.sha256.as_str()));
    assert_eq!(r.doc.logo.as_ref().unwrap().sha256, a.sha256);
    assert!(cert::canonical(&r.doc, false).contains(&format!("logo={}", a.sha256)));
}

#[test]
fn 이판_html_경력사항은_큰_칸_가운데이고_로고는_흑백_그림이다() {
    let w = world();
    set_logo(&w, &logo_a());
    let id = issue_now(&w, "제2026-001호").unwrap();
    let (doc, proof) = w.db.read(|c| for_output(c, id)).unwrap();
    let h = render::render(&doc, Mode::Issued(proof)).unwrap();
    // 경력 사항: 경력 영역 덩어리 위에 겹친 왼쪽 큰 칸(머리글 + 경력 줄)의 가로·세로 가운데.
    // 줄을 묶은 칸(rowspan)은 쓰지 않는다 — 쪽 나눔이 1판과 같아야 한다
    assert!(h.contains("<div class=\"career-zone\" data-split=\"page\"><div class=\"side-label\">경력<br>사항</div>"));
    assert!(h.contains(".career-zone .side-label { position: absolute; top: 0; bottom: 0; left: 0; width: 11%; display: flex; align-items: center; justify-content: center;"));
    assert!(!h.contains("rowspan=\"5\""), "경력 줄을 묶지 않는다");
    assert_eq!(h.matches("<tr class=\"").count(), 5, "기본 5줄");
    assert_eq!(h.matches("<td class=\"side\"></td>").count(), 5, "줄마다 왼쪽 칸(위·아래 선 없음)");
    // 로고: 표 글자 뒤(z-index -1), 머리글·경력 사항 칸을 뺀 경력 줄 영역 가운데, 종횡비 그대로 영역 안
    assert!(h.contains("<div class=\"logo-mark\"><img alt=\"\" src=\"data:image/png;base64,"));
    assert!(h.contains(".career-zone .logo-mark { position: absolute; top: 45pt; bottom: 0; left: 11%; right: 0; z-index: -1;"));
    assert!(h.contains(".career-zone .logo-mark img { display: block; max-width: min(70%, 85mm); max-height: min(calc(100% - 16pt), 85mm); }"));
    assert!(!h.contains("filter:") && !h.contains("opacity:"), "흑백·옅게는 그림 자체 — CSS 에 기대지 않는다");
    // 그림이 흑백인지 — data URL 을 풀어 본다
    let start = h.find("data:image/png;base64,").unwrap() + "data:image/png;base64,".len();
    let end = h[start..].find('"').unwrap() + start;
    let png = decode_b64(&h[start..end]);
    let img = image::load_from_memory(&png).unwrap().to_rgba8();
    assert!(img.pixels().all(|p| p.0[0] == p.0[1] && p.0[1] == p.0[2]), "흑백");
    assert!(img.pixels().filter(|p| p.0[3] > 0).all(|p| p.0[0] >= 223), "옅다");
    // 종횡비: 그림의 가로:세로 = 로고의 가로:세로
    let l = doc.logo.unwrap();
    assert_eq!(img.width() * l.height, img.height() * l.width);
}

#[test]
fn 가로로_긴_로고도_종횡비를_지킨다() {
    let w = world();
    let wide = set_logo(&w, &fake_logo(1000, 200, ImageFormat::Png));
    let id = issue_now(&w, "제2026-001호").unwrap();
    let (doc, proof) = w.db.read(|c| for_output(c, id)).unwrap();
    let h = render::render(&doc, Mode::Issued(proof)).unwrap();
    let start = h.find("data:image/png;base64,").unwrap() + "data:image/png;base64,".len();
    let end = h[start..].find('"').unwrap() + start;
    let img = image::load_from_memory(&decode_b64(&h[start..end])).unwrap();
    assert_eq!(img.width() * wide.height, img.height() * wide.width, "가로 5 : 세로 1");
    // 가로 상한(경력 줄 너비의 70%·85mm)과 세로 상한을 함께 둔다 — img 의 max-width/max-height 는 종횡비를 지킨다
    assert!(h.contains("max-width: min(70%, 85mm); max-height: min(calc(100% - 16pt), 85mm);"));
}

#[test]
fn 미리보기는_발급전_표시와_학교_로고가_함께_있다() {
    let w = world();
    set_logo(&w, &logo_a());
    let built = w.db.read(|c| drafting::prepare(c, request(&w, "제2026-001호"), NOW)).unwrap();
    assert!(built.doc.logo.is_some(), "미리보기도 지금 로고");
    let h = render::render(&built.doc, Mode::PreviewDraft).unwrap();
    assert!(h.contains("class=\"draft-mark\"") && h.contains("<div class=\"logo-mark\"><img"));
    // 발급 전 표시(z-index 10, 붉은 글자)는 로고(z-index -1, 표 글자 뒤)보다 위
    assert!(h.contains(".draft-mark { position: fixed;") && h.contains("z-index: 10"));
}

#[test]
fn 많은_경력도_한_표로_나오고_쪽_나눔은_출력_창이_한다() {
    let w = world();
    set_logo(&w, &logo_a());
    let id = issue_now(&w, "제2026-001호").unwrap();
    let (doc, _) = w.db.read(|c| for_output(c, id)).unwrap();
    let mut many = doc.clone();
    while many.items.len() < 30 {
        many.items.push(many.items[0].clone());
    }
    let h = render::render(&many, Mode::PreviewDraft).unwrap();
    assert!(!h.contains("rowspan=\"30\""), "줄을 묶지 않는다(묶으면 경력표 전체가 다음 쪽으로 밀린다)");
    assert_eq!(h.matches("<tr class=\"item\">").count(), 30);
    // 출력 창(print-split.ts)이 쪽마다 덩어리로 나눌 표시, 덩어리끼리는 새 쪽
    assert_eq!(h.matches("data-split=\"page\"").count(), 1);
    assert!(h.contains(".career-zone + .career-zone { break-before: page;"));
    assert_eq!(h.matches("경력<br>사항").count(), 1);
}

// ---------------- 발급본 불변 ----------------

#[test]
fn 로고를_바꾸거나_지워도_옛_발급본은_발급_당시_로고다() {
    let w = world();
    let a = set_logo(&w, &logo_a());
    let first = issue_now(&w, "제2026-001호").unwrap();
    let b = set_logo(&w, &logo_b());
    assert_eq!(cert_logo(&w, first), Some(a.sha256.clone()), "A 로 발급한 것은 A");
    let second = issue_now(&w, "제2026-002호").unwrap();
    assert_eq!(cert_logo(&w, second), Some(b.sha256.clone()), "새 발급은 B");
    w.db.write(|c| remove(c, NOW)).unwrap();
    assert_eq!(cert_logo(&w, first), Some(a.sha256.clone()));
    assert_eq!(cert_logo(&w, second), Some(b.sha256.clone()));
    let third = issue_now(&w, "제2026-003호").unwrap();
    assert_eq!(cert_logo(&w, third), None, "삭제 뒤 새 발급은 로고 없음");
    assert!(w.db.read(|c| for_output(c, first)).is_ok() && w.db.read(|c| for_output(c, second)).is_ok());
}

#[test]
fn 내용_확인_뒤에_로고가_바뀌면_발급하지_않는다() {
    let w = world();
    set_logo(&w, &logo_a());
    let req = request(&w, "제2026-001호");
    let built = w.db.read(|c| drafting::prepare(c, req.clone(), NOW)).unwrap();
    let token = issuance::review_token(&built.doc);
    set_logo(&w, &logo_b());
    let acks = built.warnings.iter().map(|x| x.ack_key()).collect();
    let e = w
        .db
        .write(|c| issuance::issue(c, issuance::IssueRequest { prepare: req, review_token: token, acknowledged: acks, copied_from: None }, NOW, "local"))
        .unwrap_err();
    assert_eq!(e.code, "CERT_CHANGED");
}

// ---------------- 무결성 ----------------

#[test]
fn 로고_보관본이나_지문을_바꾸면_검증에_걸린다() {
    let w = world();
    let a = set_logo(&w, &logo_a());
    let first = issue_now(&w, "제2026-001호").unwrap();
    let b = set_logo(&w, &logo_b());
    let second = issue_now(&w, "제2026-002호").unwrap();
    sql(&w.db, "DROP TRIGGER certificates_only_void").unwrap();
    // A → B 로 바꿔치기
    sql(&w.db, &format!("UPDATE certificates SET logo_sha256 = '{}' WHERE id = {first}", b.sha256)).unwrap();
    assert_eq!(w.db.read(|c| reconstruct(c, first)).unwrap_err().code, "CERT_TAMPERED");
    // 있음 → 없음
    sql(&w.db, &format!("UPDATE certificates SET logo_sha256 = NULL WHERE id = {first}")).unwrap();
    assert_eq!(w.db.read(|c| reconstruct(c, first)).unwrap_err().code, "CERT_TAMPERED");
    sql(&w.db, &format!("UPDATE certificates SET logo_sha256 = '{}' WHERE id = {first}", a.sha256)).unwrap();
    assert!(w.db.read(|c| reconstruct(c, first)).is_ok(), "되돌리면 다시 맞다");
    // 보관본 바이트를 다른 그림으로
    sql(&w.db, "DROP TRIGGER certificate_logos_no_update").unwrap();
    w.db.write(|c| Ok(c.execute("UPDATE certificate_logos SET data = ?1 WHERE sha256 = ?2", rusqlite::params![a.png.as_slice(), b.sha256])?))
        .unwrap();
    assert_eq!(w.db.read(|c| reconstruct(c, second)).unwrap_err().code, "CERT_TAMPERED");
}

#[test]
fn 발급본_로고는_db_가_고치지도_지우지도_못하게_한다() {
    let w = world();
    let a = set_logo(&w, &logo_a());
    let id = issue_now(&w, "제2026-001호").unwrap();
    assert!(sql(&w.db, &format!("UPDATE certificates SET logo_sha256 = NULL WHERE id = {id}")).is_err());
    assert!(sql(&w.db, "DELETE FROM certificate_logos").is_err());
    assert!(sql(&w.db, &format!("UPDATE certificate_logos SET width = 1 WHERE sha256 = '{}'", a.sha256)).is_err());
}

// ---------------- 1판 호환 ----------------

/// v0.1.1 이 만들던 1판 발급본을 흉내 낸다 (로고 없음, 1판 정규 표현)
fn issue_v1(w: &World, no: &str) -> i64 {
    let built = w.db.read(|c| drafting::prepare(c, request(w, no), NOW)).unwrap();
    let mut doc = built.doc;
    doc.template_version = 1;
    doc.logo = None;
    w.db.write(|c| store(c, &doc, &cert::issue_no_key(&doc.issue_no), w.who, None, NOW, "local")).unwrap()
}

#[test]
fn 일판_발급본은_로고를_등록해도_일판_그대로다() {
    let w = world();
    let old = issue_v1(&w, "제2026-001호");
    let before = w.db.read(|c| reconstruct(c, old)).unwrap();
    let (d0, p0) = w.db.read(|c| for_output(c, old)).unwrap();
    let html_before = render::render(&d0, Mode::Issued(p0)).unwrap();
    set_logo(&w, &logo_a());
    let after = w.db.read(|c| reconstruct(c, old)).unwrap();
    assert_eq!(after.doc, before.doc);
    assert_eq!(after.row.doc_hash, before.row.doc_hash, "doc_hash 그대로 검증");
    assert_eq!(after.doc.template_version, 1);
    assert!(after.doc.logo.is_none(), "지금 로고를 넣지 않는다");
    let (d1, p1) = w.db.read(|c| for_output(c, old)).unwrap();
    let html_after = render::render(&d1, Mode::Issued(p1)).unwrap();
    assert_eq!(html_after, html_before, "1판 출력 그대로");
    assert!(!html_after.contains("career-zone") && html_after.contains("rowspan=\"2\">경력<br>사항</th>"), "1판 배치");
    // 1판에 로고를 붙이면 DB 가 막는다(트리거를 풀어도 CHECK 가 막는다)
    sql(&w.db, "DROP TRIGGER certificates_only_void").unwrap();
    let sha = w.db.read(crate::repo::logo::current).unwrap().unwrap().sha256;
    assert!(sql(&w.db, &format!("UPDATE certificates SET logo_sha256 = '{sha}' WHERE id = {old}")).is_err());
}

// ---------------- 백업·복원 ----------------

#[test]
fn 내부_백업에_학교_로고와_발급본_로고가_들어간다() {
    let dir = tmp_dir("logo-backup");
    let w = world_in(Db::open(&dir.join(DB_FILE)).unwrap());
    let a = set_logo(&w, &logo_a());
    let id = issue_now(&w, "제2026-001호").unwrap();
    let e = backup::create(&w.db, backup::Kind::Manual, chrono::Local::now()).unwrap();
    let copy = Db::open(&e.path).unwrap();
    assert_eq!(copy.read(crate::repo::logo::current).unwrap().unwrap().sha256, a.sha256);
    assert_eq!(copy.read(|c| reconstruct(c, id)).unwrap().doc.logo.unwrap().sha256, a.sha256);
}

#[test]
fn 이동용_백업_복원_뒤에도_학교_로고와_발급본_로고가_그대로다() {
    let dir = tmp_dir("logo-portable");
    let w = world_in(Db::open(&dir.join(DB_FILE)).unwrap());
    let a = set_logo(&w, &logo_a());
    let id = issue_now(&w, "제2026-001호").unwrap();
    w.db.write(|c| recovery::set(c, &pw(PW), &pw(PW), NOW)).unwrap();
    // 다른 PC 흉내 — 이 PC 가 못 푸는 DPAPI 사본으로 만든 백업
    w.db.write(|c| Ok(c.execute("UPDATE key_store SET dpapi_blob = X'0102'", [])?)).unwrap();
    let bytes = package(&w.db);
    set_logo(&w, &logo_b()); // 백업 뒤 로고 변경 — 복원하면 A 로 돌아가야 한다
    let path = w.db.path().to_path_buf();
    let prepared = prepare(&bytes, &pw(PW)).unwrap();
    restore::stage(&prepared, &path, NOW).unwrap();
    drop(prepared);
    drop(w);
    assert!(matches!(restore::apply_pending(&path, NOW), restore::Applied::Done { .. }));
    let db = Db::open(&path).unwrap();
    assert_eq!(db.read(crate::repo::logo::current).unwrap().unwrap().sha256, a.sha256, "학교 로고 A");
    let (doc, proof) = db.read(|c| for_output(c, id)).unwrap();
    assert_eq!(doc.logo.as_ref().unwrap().sha256, a.sha256, "발급본 로고 A");
    assert!(render::render(&doc, Mode::Issued(proof)).unwrap().contains("<div class=\"logo-mark\"><img"));
}

fn decode_b64(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let b: Vec<u8> = s.bytes().filter(|c| *c != b'=').collect();
    let mut out = Vec::new();
    for ch in b.chunks(4) {
        let n = ch.iter().enumerate().fold(0u32, |acc, (i, c)| acc | (val(*c) as u32) << (18 - 6 * i));
        out.push((n >> 16) as u8);
        if ch.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if ch.len() > 3 {
            out.push(n as u8);
        }
    }
    out
}
