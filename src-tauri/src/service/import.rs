//! 엑셀 가져오기 — **강사와 경력만.** 기존 엑셀에 남은 과거 발급번호·발급 기록은 가져오지 않는다
//! (발급이력은 불변 스냅샷·암호화·doc_hash·출력 이력을 보장하는 기록이라 그런 보장이 없는 자료를 섞지 않는다).
//!
//! ```text
//!   파일 선택(.xlsx·.xlsm) → read: 읽기 전용 분석(calamine — 매크로를 실행할 기능 자체가 없다)
//!     → 시트·머리글 줄·열 자동 대응 → **대응한 열만** 꺼낸다(주민번호·주소 같은 다른 열은 읽어 두지 않는다)
//!   analyze: 줄마다 판정(정상·경고·검토 필요·오류·중복) + 성명별 묶음과 기존 강사 후보
//!   담당자 결정: 묶음마다 기존 강사와 연결 / 새 강사로 등록 / 가져오지 않음, 검토 줄은 재직중으로 넣을지
//!   plan: 결정대로 실제로 넣을 것을 계산(기존 경력과 겹치면 건너뜀) — 반영 미리보기와 지문(fingerprint)
//!   apply: **같은 계산을 다시 해서 지문이 같을 때만** 한 트랜잭션으로 반영 + imports 기록
//!          (적용 직전 before_import_ 백업은 부르는 쪽이 트랜잭션 밖에서 만든다)
//! ```
//!
//! 자동 병합을 하지 않는다: 같은 이름의 기존 강사가 있으면 — 한 명뿐이어도, 전화번호가 같아도 — 담당자가 고른다.
//! 날짜는 Phase 2 의 `date::parse_loose`·`is_current_word` 와 엑셀 날짜 숫자(`from_excel_serial`)로 읽는다.
//! 빈 종료일을 재직중이라고 단정하지 않는다 — 검토 필요로 두고 담당자가 고른 줄만 재직중으로 넣는다.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use calamine::{Data, ExcelDateTime, ExcelDateTimeType, Reader, Xlsx};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::domain::audit::Action;
use crate::domain::career::{self, CareerFields, CareerInput, CareerStatus, EndReason};
use crate::domain::date;
use crate::domain::instructor::{self, InstructorFields, InstructorInput};
use crate::error::{AppError, AppResult};
use crate::repo::{audit, career as career_repo, instructor as instructor_repo};

pub const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
const HEADER_SCAN_ROWS: usize = 20;

// ---------------------------------------------------------------
// 읽기
// ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Field {
    Name,
    Program,
    Position,
    Start,
    End,
    Duty,
    Phone,
}

impl Field {
    pub const ALL: [Field; 7] = [Field::Name, Field::Program, Field::Position, Field::Start, Field::End, Field::Duty, Field::Phone];
    const REQUIRED: [Field; 6] = [Field::Name, Field::Program, Field::Position, Field::Start, Field::End, Field::Duty];

    pub fn label(self) -> &'static str {
        match self {
            Field::Name => "성명",
            Field::Program => "프로그램명",
            Field::Position => "직위",
            Field::Start => "시작날짜",
            Field::End => "마감날짜",
            Field::Duty => "지도사항",
            Field::Phone => "연락처",
        }
    }

    /// 머리글 이름 (공백을 뺀 뒤 비교)
    fn aliases(self) -> &'static [&'static str] {
        match self {
            Field::Name => &["성명", "이름", "강사명", "강사성명", "강사이름"],
            Field::Program => &["프로그램명", "프로그램", "강좌명", "강좌"],
            Field::Position => &["직위", "직책"],
            Field::Start => &["시작날짜", "시작일", "시작일자", "계약시작일", "시작", "부터"],
            Field::End => &["마감날짜", "마감일", "종료날짜", "종료일", "종료일자", "계약종료일", "종료", "마감", "까지"],
            Field::Duty => &["지도사항", "지도내용", "담당업무"],
            Field::Phone => &["연락처", "전화번호", "휴대폰", "휴대전화", "핸드폰"],
        }
    }
}

/// 셀 값 (대응한 열만)
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Empty,
    Text(String),
    /// 날짜 형식이 아닌 숫자 칸
    Number(f64),
    /// 날짜 형식 칸의 일련번호
    Date(f64),
}

impl Cell {
    fn text(&self) -> String {
        match self {
            Cell::Empty => String::new(),
            Cell::Text(s) => s.trim().to_string(),
            Cell::Number(n) | Cell::Date(n) => {
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Cell::Empty => true,
            Cell::Text(s) => s.trim().is_empty(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SourceKind {
    #[serde(rename = "XLSX")]
    Xlsx,
    #[serde(rename = "XLSM")]
    Xlsm,
}

impl SourceKind {
    pub fn code(self) -> &'static str {
        match self {
            SourceKind::Xlsx => "XLSX",
            SourceKind::Xlsm => "XLSM",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawRow {
    /// 엑셀 줄 번호 (1부터)
    pub row_no: u32,
    pub cells: HashMap<Field, Cell>,
}

/// 파일에서 꺼낸 것 — 대응한 열의 값뿐이다.
#[derive(Debug, Clone)]
pub struct Extract {
    pub kind: SourceKind,
    pub sha256: String,
    pub sheets: Vec<String>,
    pub sheet: String,
    pub header_row: u32,
    /// (항목, 엑셀 머리글, 열 글자)
    pub columns: Vec<(Field, String, String)>,
    pub rows: Vec<RawRow>,
    pub date1904: bool,
}

fn col_letter(mut c: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push((b'A' + (c % 26) as u8) as char);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.iter().rev().collect()
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn to_cell(d: &Data, date1904: &mut bool) -> Cell {
    match d {
        Data::Empty => Cell::Empty,
        Data::String(s) => Cell::Text(s.clone()),
        Data::Int(i) => Cell::Number(*i as f64),
        Data::Float(f) => Cell::Number(*f),
        Data::Bool(b) => Cell::Text(b.to_string()),
        Data::DateTime(dt) => {
            if *dt == ExcelDateTime::new(dt.as_f64(), ExcelDateTimeType::DateTime, true) {
                *date1904 = true;
            }
            Cell::Date(dt.as_f64())
        }
        Data::DateTimeIso(s) => Cell::Text(s.chars().take(10).collect()),
        Data::DurationIso(s) => Cell::Text(s.clone()),
        Data::Error(_) => Cell::Text("#오류".into()),
    }
}

fn import_err(msg: impl Into<String>) -> AppError {
    AppError::new("IMPORT_INVALID", msg)
}

/// 파일을 읽는다(읽기 전용). `sheet` 를 주지 않으면 `강사명단` → 머리글을 찾을 수 있는 첫 시트.
pub fn read(path: &Path, sheet: Option<&str>) -> AppResult<Extract> {
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    let kind = match ext.as_deref() {
        Some("xlsx") => SourceKind::Xlsx,
        Some("xlsm") => SourceKind::Xlsm,
        _ => return Err(import_err("엑셀 통합 문서(.xlsx·.xlsm)만 가져올 수 있습니다.")),
    };
    let size = std::fs::metadata(path)?.len();
    if size > MAX_FILE_BYTES {
        return Err(import_err("파일이 너무 큽니다(20MB 이하만)."));
    }
    let bytes = std::fs::read(path)?;
    read_bytes(&bytes, kind, sheet)
}

pub fn read_bytes(bytes: &[u8], kind: SourceKind, sheet: Option<&str>) -> AppResult<Extract> {
    let sha256 = crate::crypto::plain_sha256(bytes);
    let mut wb: Xlsx<_> = Xlsx::new(std::io::Cursor::new(bytes))
        .map_err(|_| import_err("엑셀 파일을 열지 못했습니다. 암호가 걸렸거나 손상된 파일일 수 있습니다."))?;
    let sheets = wb.sheet_names().to_vec();
    if sheets.is_empty() {
        return Err(import_err("시트가 없는 파일입니다."));
    }
    let order: Vec<String> = match sheet {
        Some(s) => {
            if !sheets.iter().any(|x| x == s) {
                return Err(import_err("고른 시트를 찾을 수 없습니다."));
            }
            vec![s.to_string()]
        }
        None => {
            let mut v: Vec<String> = sheets.iter().filter(|s| norm(s) == "강사명단").cloned().collect();
            v.extend(sheets.iter().filter(|s| norm(s) != "강사명단").cloned());
            v
        }
    };
    let mut last_missing: Vec<&str> = Vec::new();
    for name in &order {
        let range = wb.worksheet_range(name).map_err(|_| import_err("시트를 읽지 못했습니다."))?;
        let (r0, c0) = range.start().unwrap_or((0, 0));
        let rows: Vec<&[Data]> = range.rows().collect();
        // 머리글 줄 찾기
        let mut found: Option<(usize, HashMap<Field, usize>)> = None;
        for (i, row) in rows.iter().enumerate().take(HEADER_SCAN_ROWS) {
            let mut map = HashMap::new();
            for (j, d) in row.iter().enumerate() {
                if let Data::String(s) = d {
                    let n = norm(s);
                    for f in Field::ALL {
                        if !map.contains_key(&f) && f.aliases().contains(&n.as_str()) {
                            map.insert(f, j);
                        }
                    }
                }
            }
            if map.contains_key(&Field::Name) && map.contains_key(&Field::Start) {
                found = Some((i, map));
                break;
            }
        }
        let Some((hi, map)) = found else {
            last_missing = vec!["성명", "시작날짜"];
            continue;
        };
        let missing: Vec<&str> = Field::REQUIRED.iter().filter(|f| !map.contains_key(f)).map(|f| f.label()).collect();
        if !missing.is_empty() {
            if sheet.is_some() || order.len() == 1 {
                return Err(import_err(format!("'{name}' 시트에 필요한 열이 없습니다: {}", missing.join(", "))));
            }
            last_missing = missing;
            continue;
        }
        let mut columns: Vec<(Field, String, String)> = map
            .iter()
            .map(|(f, j)| {
                let header = match rows[hi].get(*j) {
                    Some(Data::String(s)) => s.trim().to_string(),
                    _ => String::new(),
                };
                (*f, header, col_letter(c0 + *j as u32))
            })
            .collect();
        columns.sort();
        let mut date1904 = false;
        let mut out = Vec::new();
        for (i, row) in rows.iter().enumerate().skip(hi + 1) {
            let cells: HashMap<Field, Cell> = map
                .iter()
                .map(|(f, j)| (*f, row.get(*j).map(|d| to_cell(d, &mut date1904)).unwrap_or(Cell::Empty)))
                .collect();
            if cells.values().all(Cell::is_empty) {
                continue; // 빈 줄
            }
            out.push(RawRow { row_no: r0 + i as u32 + 1, cells });
        }
        return Ok(Extract {
            kind,
            sha256,
            sheets: sheets.clone(),
            sheet: name.clone(),
            header_row: r0 + hi as u32 + 1,
            columns,
            rows: out,
            date1904,
        });
    }
    Err(import_err(format!(
        "강사 경력 목록을 찾지 못했습니다. 머리글 줄에 {} 열이 있어야 합니다.",
        if last_missing.is_empty() { "성명·시작날짜".to_string() } else { last_missing.join("·") }
    )))
}

// ---------------------------------------------------------------
// 줄 판정
// ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RowStatus {
    Ok,
    /// 가져오지만 확인할 점이 있다
    Warn,
    /// 종료일이 비어 있다 — 담당자가 재직중으로 넣을지 고른다
    Review,
    /// 가져올 수 없다
    Error,
    /// 파일 안에서 같은 경력이 앞에 있다
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    Date(NaiveDate),
    /// `현재`·`재직중`·`재직`·`근무중`
    Current,
    Blank,
    Bad,
}

#[derive(Debug, Clone)]
pub struct Parsed {
    pub row_no: u32,
    pub name: String,
    pub program: String,
    pub position: String,
    pub duty: String,
    pub phone: String,
    pub start: Option<NaiveDate>,
    pub end: End,
    pub start_text: String,
    pub end_text: String,
    pub status: RowStatus,
    pub messages: Vec<String>,
}

/// 날짜 칸 읽기 — (날짜, 읽은 방식)
fn read_date(c: &Cell, date1904: bool) -> Result<(NaiveDate, &'static str), String> {
    match c {
        Cell::Empty => Err("비어 있습니다".into()),
        Cell::Date(n) => date::from_excel_serial(*n, date1904).map(|d| (d, "엑셀 날짜")).map_err(|e| e.message()),
        Cell::Number(n) => {
            // 20220304 처럼 숫자로 적은 날짜, 아니면 날짜 형식이 빠진 엑셀 날짜 숫자
            if n.fract() == 0.0 && (19_500_101.0..=20_991_231.0).contains(n) {
                date::parse_loose(&format!("{}", *n as i64)).map(|d| (d, "숫자 8자리")).map_err(|e| e.message())
            } else {
                date::from_excel_serial(*n, date1904).map(|d| (d, "엑셀 날짜 숫자")).map_err(|e| e.message())
            }
        }
        Cell::Text(s) => date::parse_loose(s).map(|d| (d, "글자 날짜")).map_err(|e| e.message()),
    }
}

/// 분석 결과
#[derive(Debug, Clone)]
pub struct Analysis {
    pub rows: Vec<Parsed>,
    pub groups: Vec<Group>,
    /// 날짜를 읽은 방식별 건수
    pub formats: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: i64,
    pub distinguisher: String,
    pub phone: String,
    pub careers: i64,
    pub programs: Vec<String>,
}

/// 성명 하나에 딸린 줄 묶음
#[derive(Debug, Clone)]
pub struct Group {
    pub name: String,
    pub row_nos: Vec<u32>,
    /// 파일에 적힌 연락처(겹치지 않게) — 참고용
    pub phones: Vec<String>,
    /// 같은 이름의 기존 강사(보관 제외) — 자동으로 고르지 않는다
    pub candidates: Vec<Candidate>,
}

pub fn analyze(conn: &Connection, ex: &Extract, today: NaiveDate) -> AppResult<Analysis> {
    let mut rows = Vec::with_capacity(ex.rows.len());
    let mut formats: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen: HashSet<(String, String, Option<NaiveDate>)> = HashSet::new();
    let mut active_per_name: HashMap<String, usize> = HashMap::new();

    for raw in &ex.rows {
        let get = |f: Field| raw.cells.get(&f).cloned().unwrap_or(Cell::Empty);
        let mut p = Parsed {
            row_no: raw.row_no,
            name: get(Field::Name).text(),
            program: get(Field::Program).text(),
            position: get(Field::Position).text(),
            duty: get(Field::Duty).text(),
            phone: get(Field::Phone).text(),
            start: None,
            end: End::Blank,
            start_text: get(Field::Start).text(),
            end_text: get(Field::End).text(),
            status: RowStatus::Ok,
            messages: Vec::new(),
        };
        let mut errors = Vec::new();
        for (f, v) in [(Field::Name, &p.name), (Field::Program, &p.program), (Field::Position, &p.position), (Field::Duty, &p.duty)] {
            if v.is_empty() {
                errors.push(format!("{}이(가) 비어 있습니다.", f.label()));
            }
        }
        let start_cell = get(Field::Start);
        match read_date(&start_cell, ex.date1904) {
            Ok((d, how)) => {
                p.start = Some(d);
                *formats.entry(how.into()).or_default() += 1;
            }
            Err(m) => errors.push(format!("시작날짜: {m}")),
        }
        let end_cell = get(Field::End);
        p.end = if end_cell.is_empty() {
            *formats.entry("빈 종료일".into()).or_default() += 1;
            End::Blank
        } else if matches!(&end_cell, Cell::Text(s) if date::is_current_word(s)) {
            *formats.entry("현재·재직중".into()).or_default() += 1;
            End::Current
        } else {
            match read_date(&end_cell, ex.date1904) {
                Ok((d, how)) => {
                    *formats.entry(how.into()).or_default() += 1;
                    End::Date(d)
                }
                Err(m) => {
                    errors.push(format!("마감날짜: {m}"));
                    End::Bad
                }
            }
        };
        if let (Some(s), End::Date(e)) = (p.start, &p.end) {
            if s > *e {
                errors.push("시작날짜가 마감날짜보다 늦습니다.".into());
            }
        }
        // domain 규칙(길이 등)도 미리 본다
        if errors.is_empty() {
            if let Err(e) = career::prepare(career_input(&p)) {
                errors.push(e.user_message);
            }
        }
        if !errors.is_empty() {
            p.status = RowStatus::Error;
            p.messages = errors;
            rows.push(p);
            continue;
        }
        let key = (p.name.clone(), p.program.clone(), p.start);
        if !seen.insert(key) {
            p.status = RowStatus::Duplicate;
            p.messages.push("같은 성명·프로그램·시작날짜의 줄이 위에 있습니다.".into());
            rows.push(p);
            continue;
        }
        match &p.end {
            End::Blank => {
                p.status = RowStatus::Review;
                p.messages.push("마감날짜가 비어 있습니다. 재직중으로 가져올지 확인해 주세요.".into());
            }
            End::Date(e) if *e > today => {
                p.status = RowStatus::Warn;
                p.messages.push(format!("마감날짜({})가 오늘 이후입니다. 종료(계약만료)로 가져옵니다.", date::display(*e)));
            }
            _ => {}
        }
        if let Some(s) = p.start {
            if s > today {
                p.status = if p.status == RowStatus::Review { RowStatus::Review } else { RowStatus::Warn };
                p.messages.push("시작날짜가 오늘 이후입니다.".into());
            }
        }
        if p.end == End::Current {
            *active_per_name.entry(p.name.clone()).or_default() += 1;
        }
        rows.push(p);
    }
    for p in rows.iter_mut() {
        if p.end == End::Current && active_per_name.get(&p.name).copied().unwrap_or(0) > 1 {
            if p.status == RowStatus::Ok {
                p.status = RowStatus::Warn;
            }
            p.messages.push("같은 성명의 재직중 경력이 여럿입니다.".into());
        }
    }

    // 성명별 묶음 (파일에 나온 차례)
    let mut groups: Vec<Group> = Vec::new();
    for p in rows.iter().filter(|p| !p.name.is_empty()) {
        if let Some(g) = groups.iter_mut().find(|g| g.name == p.name) {
            g.row_nos.push(p.row_no);
            if !p.phone.is_empty() && !g.phones.contains(&p.phone) {
                g.phones.push(p.phone.clone());
            }
        } else {
            groups.push(Group {
                name: p.name.clone(),
                row_nos: vec![p.row_no],
                phones: if p.phone.is_empty() { vec![] } else { vec![p.phone.clone()] },
                candidates: candidates(conn, &p.name)?,
            });
        }
    }
    Ok(Analysis { rows, groups, formats })
}

fn candidates(conn: &Connection, name: &str) -> AppResult<Vec<Candidate>> {
    let list = instructor_repo::search(
        conn,
        &instructor_repo::Filter { query: name.to_string(), scope: instructor_repo::Scope::All },
    )?;
    Ok(list
        .into_iter()
        .filter(|s| s.instructor.name == name && s.instructor.archived_at.is_none())
        .map(|s| Candidate {
            id: s.instructor.id,
            distinguisher: s.instructor.distinguisher,
            phone: s.instructor.phone,
            careers: s.careers,
            programs: s.programs,
        })
        .collect())
}

/// 줄 → 경력 입력. 종료일이 날짜면 종료(계약만료), 아니면 재직중.
/// 빈 종료일 줄은 담당자가 재직중으로 고른 경우에만 여기까지 온다(`plan`).
fn career_input(p: &Parsed) -> CareerInput {
    let (status, end_date, end_reason) = match &p.end {
        End::Date(d) => (CareerStatus::Ended, Some(date::to_iso(*d)), Some(EndReason::ContractEnd)),
        End::Current | End::Blank | End::Bad => (CareerStatus::Active, None, None),
    };
    CareerInput {
        program_name: p.program.clone(),
        position: p.position.clone(),
        duty: p.duty.clone(),
        start_date: p.start.map(date::to_iso).unwrap_or_default(),
        status,
        end_date,
        end_reason,
        planned_end_date: None,
        memo: String::new(),
    }
}

// ---------------------------------------------------------------
// 담당자 결정 → 반영 계획
// ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GroupAction {
    /// 기존 강사와 연결
    Link,
    /// 새 강사로 등록
    New,
    /// 가져오지 않음
    Skip,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDecision {
    pub name: String,
    pub action: Option<GroupAction>,
    #[serde(default)]
    pub instructor_id: Option<i64>,
    /// 새 강사의 구분 메모
    #[serde(default)]
    pub distinguisher: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decisions {
    pub groups: Vec<GroupDecision>,
    /// 검토 줄 중 재직중으로 가져올 줄 번호
    #[serde(default)]
    pub review_active: Vec<u32>,
    /// 담당자가 뺀 줄 번호
    #[serde(default)]
    pub excluded_rows: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Existing(i64),
    New(InstructorFields),
}

#[derive(Debug, Clone)]
pub struct Op {
    pub target: Target,
    pub careers: Vec<(u32, CareerFields)>,
}

/// 줄마다 어떻게 되는가
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Outcome {
    Add,
    SkipError,
    SkipDuplicateFile,
    /// 연결한 기존 강사에게 같은 프로그램·시작날짜 경력이 이미 있다
    SkipDuplicateDb,
    /// 검토 줄인데 재직중으로 고르지 않았다
    SkipReview,
    SkipExcluded,
    SkipGroup,
    /// 강사를 아직 고르지 않았다(연결·새 강사·가져오지 않음) — 반영할 수 없다
    PendingDecision,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub rows_total: usize,
    pub instructors_created: usize,
    pub instructors_linked: usize,
    pub careers_added: usize,
    pub rows_warned: usize,
    pub rows_skipped: usize,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub ops: Vec<Op>,
    pub outcomes: BTreeMap<u32, Outcome>,
    pub summary: Summary,
    /// 이대로는 반영할 수 없는 까닭 (비어 있어야 반영)
    pub problems: Vec<String>,
    /// 반영할 내용의 지문 — 적용 때 다시 계산해 같아야 한다
    pub fingerprint: String,
}

pub fn plan(conn: &Connection, a: &Analysis, d: &Decisions) -> AppResult<Plan> {
    let mut problems = Vec::new();
    let mut outcomes = BTreeMap::new();
    let mut ops = Vec::new();
    let mut summary = Summary { rows_total: a.rows.len(), ..Default::default() };
    let by_row: HashMap<u32, &Parsed> = a.rows.iter().map(|p| (p.row_no, p)).collect();
    let excluded: HashSet<u32> = d.excluded_rows.iter().copied().collect();
    let review_active: HashSet<u32> = d.review_active.iter().copied().collect();

    for p in &a.rows {
        if p.name.is_empty() {
            outcomes.insert(p.row_no, Outcome::SkipError);
        }
    }
    for g in &a.groups {
        let dec = d.groups.iter().find(|x| x.name == g.name);
        let action = dec.and_then(|x| x.action);
        let target = match action {
            None => {
                problems.push(format!("'{}': 기존 강사와 연결할지, 새 강사로 등록할지, 가져오지 않을지 골라 주세요.", g.name));
                None
            }
            Some(GroupAction::Skip) => None,
            Some(GroupAction::Link) => match dec.and_then(|x| x.instructor_id) {
                Some(id) if g.candidates.iter().any(|c| c.id == id) => Some(Target::Existing(id)),
                _ => {
                    problems.push(format!("'{}': 연결할 기존 강사를 골라 주세요.", g.name));
                    None
                }
            },
            Some(GroupAction::New) => {
                let dist = dec.map(|x| x.distinguisher.trim().to_string()).unwrap_or_default();
                if !g.candidates.is_empty() && dist.is_empty() {
                    problems.push(format!("'{}': 같은 이름의 강사가 이미 있습니다. 새 강사로 등록하려면 구분 메모를 적어 주세요.", g.name));
                }
                let phone = if g.phones.len() == 1 { g.phones[0].clone() } else { String::new() };
                match instructor::prepare(InstructorInput { name: g.name.clone(), distinguisher: dist, phone, memo: String::new() }) {
                    Ok(f) => Some(Target::New(f)),
                    Err(e) => {
                        problems.push(format!("'{}': {}", g.name, e.user_message));
                        None
                    }
                }
            }
        };
        let existing: HashSet<(String, NaiveDate)> = match &target {
            Some(Target::Existing(id)) => career_repo::list_by_instructor(conn, *id, true)?
                .into_iter()
                .map(|c| (c.fields.program_name, c.fields.start_date))
                .collect(),
            _ => HashSet::new(),
        };
        let mut careers = Vec::new();
        for no in &g.row_nos {
            let p = by_row[no];
            let outcome = if action.is_none() {
                Outcome::PendingDecision
            } else if action == Some(GroupAction::Skip) || target.is_none() {
                Outcome::SkipGroup
            } else if excluded.contains(no) {
                Outcome::SkipExcluded
            } else {
                match p.status {
                    RowStatus::Error => Outcome::SkipError,
                    RowStatus::Duplicate => Outcome::SkipDuplicateFile,
                    RowStatus::Review if !review_active.contains(no) => Outcome::SkipReview,
                    _ => {
                        if p.start.is_some_and(|s| existing.contains(&(p.program.clone(), s))) {
                            Outcome::SkipDuplicateDb
                        } else {
                            Outcome::Add
                        }
                    }
                }
            };
            if outcome == Outcome::Add {
                match career::prepare(career_input(p)) {
                    Ok(f) => {
                        if p.status == RowStatus::Warn || p.status == RowStatus::Review {
                            summary.rows_warned += 1;
                        }
                        careers.push((*no, f));
                    }
                    Err(e) => {
                        problems.push(format!("{}행: {}", no, e.user_message));
                    }
                }
            }
            outcomes.insert(*no, outcome);
        }
        if let Some(t) = target {
            // 넣을 경력이 없으면 강사도 만들거나 잇지 않는다
            if !careers.is_empty() {
                match &t {
                    Target::Existing(_) => summary.instructors_linked += 1,
                    Target::New(_) => summary.instructors_created += 1,
                }
                summary.careers_added += careers.len();
                ops.push(Op { target: t, careers });
            }
        }
    }
    summary.rows_skipped = outcomes.values().filter(|o| **o != Outcome::Add).count();
    if problems.is_empty() && summary.careers_added == 0 {
        problems.push("가져올 경력이 없습니다.".into());
    }
    let fingerprint = fingerprint(&ops, &a.groups);
    Ok(Plan { ops, outcomes, summary, problems, fingerprint })
}

/// 반영할 내용 + 담당자가 본 기존 강사 후보. 미리보기 뒤에 같은 이름의 강사가 생기거나 보관되면 달라진다.
fn fingerprint(ops: &[Op], groups: &[Group]) -> String {
    let mut s = String::from("IMPORTPLAN/1\n");
    for g in groups {
        let ids: Vec<i64> = g.candidates.iter().map(|c| c.id).collect();
        s.push_str(&format!("group={:?}|{ids:?}\n", g.name));
    }
    for op in ops {
        match &op.target {
            Target::Existing(id) => s.push_str(&format!("link={id}\n")),
            Target::New(f) => s.push_str(&format!("new={:?}|{:?}|{:?}\n", f.name, f.distinguisher, f.phone)),
        }
        for (no, f) in &op.careers {
            s.push_str(&format!(
                "{no}|{:?}|{:?}|{:?}|{}|{:?}\n",
                f.program_name, f.position, f.duty, f.start_date, f.term
            ));
        }
    }
    crate::crypto::plain_sha256(s.as_bytes())
}

// ---------------------------------------------------------------
// 반영
// ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub import_id: i64,
    pub summary: Summary,
}

/// 같은 계산을 다시 해서 지문이 같을 때만 한 트랜잭션으로 반영한다. `Db::write` 안에서 부른다.
pub fn apply(conn: &Connection, ex: &Extract, d: &Decisions, expected: &str, now: &str) -> AppResult<Applied> {
    let today = date::today_of(now).map_err(|_| AppError::internal("시각 형식"))?;
    let a = analyze(conn, ex, today)?;
    let p = plan(conn, &a, d)?;
    if !p.problems.is_empty() {
        return Err(AppError::new("IMPORT_NOT_READY", p.problems.join(" ")));
    }
    if p.fingerprint != expected {
        return Err(AppError::new(
            "IMPORT_CHANGED",
            "반영 미리보기 뒤에 자료나 선택이 바뀌었습니다. 가져오지 않았습니다. 미리보기를 다시 해 주세요.",
        ));
    }
    let label: String = format!("엑셀 가져오기 {} · 시트 {}", &now[..10.min(now.len())], ex.sheet).chars().take(100).collect();
    conn.execute(
        "INSERT INTO imports (uuid, created_at, label, source_kind, source_sha256, sheet_name, rows_total,
                              instructors_created, instructors_linked, careers_added, rows_warned, rows_skipped, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'local')",
        params![
            uuid::Uuid::new_v4().to_string(),
            now,
            label,
            ex.kind.code(),
            ex.sha256,
            ex.sheet.chars().take(100).collect::<String>(),
            p.summary.rows_total as i64,
            p.summary.instructors_created as i64,
            p.summary.instructors_linked as i64,
            p.summary.careers_added as i64,
            p.summary.rows_warned as i64,
            p.summary.rows_skipped as i64,
        ],
    )?;
    let import_id = conn.last_insert_rowid();
    for op in &p.ops {
        let instructor_id = match &op.target {
            Target::Existing(id) => {
                if instructor_repo::get(conn, *id)?.is_archived() {
                    return Err(AppError::new("IMPORT_CHANGED", "연결할 강사가 보관되었습니다. 미리보기를 다시 해 주세요."));
                }
                *id
            }
            Target::New(f) => {
                let id = instructor_repo::insert(conn, f, now)?;
                conn.execute("UPDATE instructors SET import_id = ?2 WHERE id = ?1", params![id, import_id])?;
                id
            }
        };
        for (_, f) in &op.careers {
            career_repo::insert(conn, instructor_id, f, Some(import_id), now)?;
        }
    }
    let s = &p.summary;
    audit::add(
        conn,
        now,
        Action::Import,
        Some(import_id),
        &format!(
            "엑셀 가져오기 · 새 강사 {} · 기존 강사 연결 {} · 경력 {} · 경고 {} · 건너뜀 {}",
            s.instructors_created, s.instructors_linked, s.careers_added, s.rows_warned, s.rows_skipped
        ),
    )?;
    Ok(Applied { import_id, summary: p.summary })
}

/// 반영 절차 전체: 미리보기와 같은지 먼저 확인 → `before_import_` 백업(트랜잭션 밖) → 한 트랜잭션으로 반영.
pub fn run(db: &crate::db::Db, ex: &Extract, d: &Decisions, expected: &str, now: &str) -> AppResult<Applied> {
    let today = date::today_of(now).map_err(|_| AppError::internal("시각 형식"))?;
    let ready = db.read(|c| {
        let a = analyze(c, ex, today)?;
        plan(c, &a, d)
    })?;
    if !ready.problems.is_empty() || ready.fingerprint != expected {
        return Err(AppError::new(
            "IMPORT_CHANGED",
            "반영 미리보기 뒤에 자료나 선택이 바뀌었습니다. 가져오지 않았습니다. 미리보기를 다시 해 주세요.",
        ));
    }
    crate::db::backup::create(db, crate::db::backup::Kind::BeforeImport, chrono::Local::now())?;
    db.write(|c| apply(c, ex, d, expected, now))
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod import_tests;
