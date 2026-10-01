use super::*;
use crate::db::Db;

const NOW: &str = "2026-10-01T09:00:00";
const LATER: &str = "2026-10-02T09:00:00";

fn input(name: &str, distinguisher: &str) -> InstructorInput {
    InstructorInput {
        name: name.into(),
        distinguisher: distinguisher.into(),
        ..Default::default()
    }
}

fn names(list: &[Summary]) -> Vec<(String, String)> {
    list.iter()
        .map(|s| (s.instructor.name.clone(), s.instructor.distinguisher.clone()))
        .collect()
}

fn filter(q: &str, include_archived: bool) -> Filter {
    Filter {
        query: q.into(),
        include_archived,
    }
}

#[test]
fn 등록하면_uuid와_시각이_붙는다() {
    let db = Db::memory();
    let a = db.write(|c| create(c, input(" 김으뜸 ", ""), NOW)).unwrap();
    let b = db.write(|c| create(c, input("박하늘", ""), NOW)).unwrap();
    assert_eq!(a.name, "김으뜸");
    assert_eq!(a.uuid.len(), 36);
    assert_ne!(a.uuid, b.uuid);
    assert_eq!(a.created_at, NOW);
    assert_eq!(a.updated_at, NOW);
    assert!(!a.is_archived());
}

#[test]
fn 동명이인을_등록하고_찾으면_둘_다_나온다() {
    let db = Db::memory();
    let a = db.write(|c| create(c, input("김으뜸", "1985년생"), NOW)).unwrap();
    let b = db.write(|c| create(c, input("김으뜸", "마술 강사"), NOW)).unwrap();
    db.write(|c| create(c, input("박하늘", ""), NOW)).unwrap();
    assert_ne!(a.id, b.id);

    let found = db.read(|c| search(c, &filter("김으뜸", false))).unwrap();
    assert_eq!(
        names(&found),
        vec![
            ("김으뜸".to_string(), "1985년생".to_string()),
            ("김으뜸".to_string(), "마술 강사".to_string())
        ],
        "같은 이름은 등록 차례로"
    );
}

#[test]
fn 이름_일부나_구분_메모로도_찾는다() {
    let db = Db::memory();
    db.write(|c| create(c, input("김으뜸", "이전 계약자"), NOW)).unwrap();
    db.write(|c| create(c, input("박하늘", ""), NOW)).unwrap();
    assert_eq!(db.read(|c| search(c, &filter("으뜸", false))).unwrap().len(), 1);
    assert_eq!(db.read(|c| search(c, &filter("계약자", false))).unwrap().len(), 1);
    assert_eq!(db.read(|c| search(c, &filter("  ", false))).unwrap().len(), 2, "빈 검색어는 전체");
    assert_eq!(
        db.read(|c| search(c, &filter("%", false))).unwrap().len(),
        0,
        "%는 와일드카드가 아니다"
    );
}

#[test]
fn 고치면_반영된다() {
    let db = Db::memory();
    let a = db.write(|c| create(c, input("김으뜸", ""), NOW)).unwrap();
    let mut v = input("김으뜸", "1985년생");
    v.phone = "000-0000-0000".into();
    let b = db.write(|c| update(c, a.id, v, LATER)).unwrap();
    assert_eq!(b.distinguisher, "1985년생");
    assert_eq!(b.phone, "000-0000-0000");
    assert_eq!(b.uuid, a.uuid, "uuid 는 바뀌지 않는다");
    assert_eq!(b.created_at, NOW);
    assert_eq!(b.updated_at, LATER);
}

#[test]
fn 보관하면_기본_목록에서_빠지고_전체_목록에는_남는다() {
    let db = Db::memory();
    let a = db.write(|c| create(c, input("김으뜸", ""), NOW)).unwrap();
    db.write(|c| create(c, input("박하늘", ""), NOW)).unwrap();

    let archived = db.write(|c| archive(c, a.id, LATER)).unwrap();
    assert_eq!(archived.archived_at.as_deref(), Some(LATER));

    assert_eq!(names(&db.read(|c| search(c, &filter("", false))).unwrap()).len(), 1);
    let all = db.read(|c| search(c, &filter("", true))).unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|s| s.instructor.id == a.id && s.instructor.is_archived()));
    assert!(db.read(|c| get(c, a.id)).is_ok(), "번호로는 언제든 꺼낼 수 있다");
}

#[test]
fn 보관을_해제하면_다시_나온다() {
    let db = Db::memory();
    let a = db.write(|c| create(c, input("김으뜸", ""), NOW)).unwrap();
    db.write(|c| archive(c, a.id, NOW)).unwrap();
    let back = db.write(|c| unarchive(c, a.id, LATER)).unwrap();
    assert_eq!(back.archived_at, None);
    assert_eq!(db.read(|c| search(c, &filter("김으뜸", false))).unwrap().len(), 1);
}

#[test]
fn 보관_상태가_맞지_않으면_오류() {
    let db = Db::memory();
    let a = db.write(|c| create(c, input("김으뜸", ""), NOW)).unwrap();
    assert_eq!(db.write(|c| unarchive(c, a.id, NOW)).unwrap_err().code, "NOT_ARCHIVED");
    db.write(|c| archive(c, a.id, NOW)).unwrap();
    assert_eq!(db.write(|c| archive(c, a.id, NOW)).unwrap_err().code, "ALREADY_ARCHIVED");
    assert_eq!(
        db.write(|c| update(c, a.id, input("새이름", ""), NOW)).unwrap_err().code,
        "ARCHIVED"
    );
    assert_eq!(db.read(|c| get(c, a.id)).unwrap().name, "김으뜸");
}

#[test]
fn 없는_강사는_찾을_수_없다() {
    let db = Db::memory();
    assert_eq!(db.read(|c| get(c, 99)).unwrap_err().code, "NOT_FOUND");
    assert_eq!(
        db.write(|c| update(c, 99, input("가", ""), NOW)).unwrap_err().code,
        "NOT_FOUND"
    );
}
