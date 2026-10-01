//! 규모 시험 (Phase 9) — 평소 시험에서는 돌지 않는다(`#[ignore]`).
//!
//! ```text
//! cargo test --release --lib perf -- --ignored --nocapture
//! ```
//!
//! 가상 자료: 강사 500명 · 경력 약 2,500건(2018~2026 학년도 누적) · 발급 5,000건(약 3% 취소) · 출력 이력 약 7,500건.
//! 발급은 실제 경로(`issuance::issue` — 내용 확인 → 암호화 → 문서 지문)로 만든다. 안전 검증을 빼지 않는다.
//! `CAREERCERT_PERF_OUT=<경로>` 를 주면 만든 자료를 그 경로에 떠 둔다(연습용 앱 화면 측정용).

use std::time::Instant;

use crate::crypto::recovery::{Kdf, KdfParams, Password};
use crate::db::testutil::tmp_dir;
use crate::db::{Db, DB_FILE};
use crate::domain::career::{CareerInput, CareerStatus, EndReason};
use crate::domain::certificate::IssueInput;
use crate::domain::instructor::InstructorInput;
use crate::domain::settings::{SettingsInput, DEFAULT_PURPOSE, DEFAULT_TITLE};
use crate::repo::{instructor as instructor_repo, settings as settings_repo};
use crate::service::certificate::{self as drafting, PrepareRequest};
use crate::service::history::{self, Filter};
use crate::service::issuance::{self, review_token, IssueRequest, OutputKind, LOCAL_ACTOR};
use crate::service::{career, dashboard, instructor, portable, recovery};

const NOW: &str = "2026-10-01T09:00:00";
const PW: &str = "규모 시험용 가상 복구 비밀번호";
const PROGRAMS: [&str; 10] = ["마술", "바둑", "요리", "코딩", "로봇과학", "생명과학", "미술", "음악", "배드민턴", "주산"];
const FAMILY: [&str; 10] = ["김", "이", "박", "최", "정", "강", "조", "윤", "장", "임"];
const GIVEN: [&str; 10] = ["가온", "나래", "다솜", "라온", "마루", "바다", "사랑", "아름", "자람", "하늘"];

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    println!("{label:<40} {:>9.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    out
}

#[test]
#[ignore]
fn perf_500명_5000건() {
    let dir = tmp_dir("perf");
    let db = Db::open(&dir.join(DB_FILE)).unwrap();
    db.write(|c| {
        settings_repo::save(
            c,
            SettingsInput {
                school_name: "○○초등학교".into(),
                certificate_title: DEFAULT_TITLE.into(),
                issuer_title: "○○초등학교장".into(),
                department: "방과후학교".into(),
                manager_name: "가상담당".into(),
                phone: "000-0000-0000".into(),
                default_purpose: DEFAULT_PURPOSE.into(),
            },
            NOW,
        )?;
        Ok(())
    })
    .unwrap();

    // ---------- 자료 만들기 ----------
    let t0 = Instant::now();
    let mut people: Vec<(i64, Vec<i64>)> = Vec::new();
    db.write(|c| {
        for i in 0..500usize {
            let name = format!("{}{}", FAMILY[i % 10], GIVEN[(i / 10) % 10]);
            let who = instructor::create(
                c,
                InstructorInput { name, distinguisher: format!("가상{:03}", i), phone: "000-0000-0000".into(), memo: "".into() },
                NOW,
            )?
            .id;
            let program = PROGRAMS[i % 10];
            let years = 3 + i % 5; // 3~7 학년도
            let mut ids = Vec::new();
            for k in 0..years {
                let y = 2026 - years as i32 + 1 + k as i32;
                let last = k + 1 == years;
                let input = CareerInput {
                    program_name: program.into(),
                    position: "강사".into(),
                    duty: format!("방과후학교 {program}"),
                    start_date: format!("{y}-03-04"),
                    status: if last { CareerStatus::Active } else { CareerStatus::Ended },
                    end_date: (!last).then(|| format!("{}-02-10", y + 1)),
                    end_reason: (!last).then_some(EndReason::ContractEnd),
                    planned_end_date: None,
                    memo: "".into(),
                };
                ids.push(career::create(c, who, input, false, NOW)?.id);
            }
            people.push((who, ids));
        }
        Ok(())
    })
    .unwrap();
    let careers: usize = people.iter().map(|p| p.1.len()).sum();
    println!("강사 500명 · 경력 {careers}건 만들기: {:.1} s", t0.elapsed().as_secs_f64());

    let t1 = Instant::now();
    let mut ids = Vec::new();
    for n in 0..5000usize {
        let (who, careers) = &people[n % 500];
        let req = PrepareRequest {
            instructor_id: *who,
            career_ids: careers.clone(),
            issue: IssueInput {
                rrn: "900101-1000001".into(), // privacy:fake
                address: "가상시 연습구 시험로 1".into(),
                issue_no: format!("제2026-{:05}호", n + 1),
                purpose: "기관제출".into(),
                issued_on: "2026-10-01".into(),
                mask_rrn: n % 4 == 0,
            },
        };
        let id = db
            .write(|c| {
                let built = drafting::prepare(c, req.clone(), NOW)?;
                let token = review_token(&built.doc);
                let acks = built.warnings.iter().map(|w| w.ack_key()).collect();
                issuance::issue(c, IssueRequest { prepare: req, review_token: token, acknowledged: acks, copied_from: None }, NOW, LOCAL_ACTOR)
            })
            .unwrap();
        ids.push(id);
    }
    db.write(|c| {
        for (n, id) in ids.iter().enumerate() {
            issuance::record_output(c, *id, OutputKind::Pdf, None, true, "", NOW)?;
            if n % 2 == 0 {
                issuance::record_output(c, *id, OutputKind::Print { copies: 1 }, Some("가상 프린터"), true, "", NOW)?;
            }
            if n % 33 == 0 {
                issuance::void(c, *id, "가상 취소", NOW)?;
            }
        }
        recovery::set(c, &Password::new(PW), &Password::new(PW), NOW)
    })
    .unwrap();
    println!("발급 5,000건 · 출력 이력 · 취소 만들기: {:.1} s", t1.elapsed().as_secs_f64());

    // ---------- 측정 ----------
    println!("--- 측정 (각 1회) ---");
    time("대시보드 요약(overview)", || db.read(|c| dashboard::overview(c, NOW)).unwrap());
    time("대시보드 최근 발급·취소", || db.read(|c| history::recent(c, 5)).unwrap());
    let all = time("강사 목록(전체)", || {
        db.read(|c| instructor_repo::search(c, &instructor_repo::Filter { query: String::new(), scope: instructor_repo::Scope::All })).unwrap()
    });
    assert_eq!(all.len(), 500);
    time("강사 검색('하늘')", || {
        db.read(|c| instructor_repo::search(c, &instructor_repo::Filter { query: "하늘".into(), scope: instructor_repo::Scope::All })).unwrap()
    });
    let page = time("발급이력 첫 목록(최대 300건)", || db.read(|c| history::search(c, &Filter::default())).unwrap());
    assert!(page.truncated);
    time("발급이력 검색(발급번호 일부)", || db.read(|c| history::search(c, &Filter { query: "02500".into(), ..Default::default() })).unwrap());
    time("발급이력 검색(성명)", || db.read(|c| history::search(c, &Filter { query: "김가온".into(), ..Default::default() })).unwrap());
    time("발급 상세(복호화 없음)", || db.read(|c| history::detail(c, ids[2500])).unwrap());
    time("정식 출력 준비 1건(복호화·지문)", || db.read(|c| issuance::for_output(c, ids[2501])).unwrap());

    let kdf = Kdf::fresh(KdfParams::V1);
    time("Argon2id 1판 인자 1회(64MiB·3)", || kdf.derive(&Password::new(PW)).unwrap());
    let pkg = time("이동용 백업 만들기(시험 KDF 인자)", || db.read(|c| portable::build(c, &Password::new(PW), NOW, "0.1.0")).unwrap());
    println!("  패키지 크기 {:.1} MB", pkg.len() as f64 / 1048576.0);
    let n = time("발급본 5,000건 전부 복호화·지문 검증", || db.read(portable::verify_certificates).unwrap());
    assert_eq!(n, 5000);
    let prepared = time("복원 준비 전체(풀기·검사·재포장·5,000건 검증)", || portable::prepare(&pkg, &Password::new(PW)).unwrap());
    assert_eq!(prepared.verified_certificates, 5000);

    if let Some(out) = std::env::var_os("CAREERCERT_PERF_OUT") {
        db.backup_to(std::path::Path::new(&out)).unwrap();
        println!("자료를 떠 둠: {}", std::path::Path::new(&out).display());
    }
}
