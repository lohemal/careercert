//! 변경 기록에 쓰는 말.
//!
//! summary 에는 **값을 적지 않는다** — 바뀐 항목 이름, 건수, 종료일·사유처럼 업무를 되짚는 데
//! 필요한 것만. 이름·연락처·메모 내용이 들어가면 로그가 강사 자료의 복제본이 된다.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    InstructorCreate,
    InstructorUpdate,
    InstructorArchive,
    InstructorUnarchive,
    CareerCreate,
    CareerUpdate,
    CareerEnd,
    CareerArchive,
    CareerUnarchive,
    BulkEnd,
}

impl Action {
    pub fn code(self) -> &'static str {
        match self {
            Action::InstructorCreate => "INSTRUCTOR_CREATE",
            Action::InstructorUpdate => "INSTRUCTOR_UPDATE",
            Action::InstructorArchive => "INSTRUCTOR_ARCHIVE",
            Action::InstructorUnarchive => "INSTRUCTOR_UNARCHIVE",
            Action::CareerCreate => "CAREER_CREATE",
            Action::CareerUpdate => "CAREER_UPDATE",
            Action::CareerEnd => "CAREER_END",
            Action::CareerArchive => "CAREER_ARCHIVE",
            Action::CareerUnarchive => "CAREER_UNARCHIVE",
            Action::BulkEnd => "BULK_END",
        }
    }

    pub fn target_type(self) -> &'static str {
        match self {
            Action::InstructorCreate
            | Action::InstructorUpdate
            | Action::InstructorArchive
            | Action::InstructorUnarchive => "instructor",
            Action::CareerCreate
            | Action::CareerUpdate
            | Action::CareerEnd
            | Action::CareerArchive
            | Action::CareerUnarchive => "career",
            Action::BulkEnd => "bulk_end",
        }
    }
}

/// `변경 항목: 이름, 메모` — 값 없이 항목 이름만.
pub fn changed_summary(labels: &[&str]) -> String {
    if labels.is_empty() {
        "변경 없음".into()
    } else {
        format!("변경 항목: {}", labels.join(", "))
    }
}
