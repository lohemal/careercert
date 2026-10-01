-- 003_instructors_careers — Phase 2
--
-- 강사(instructors)와 경력(careers). **지금 자료** 쪽 표다 — 고칠 수 있다.
-- 발급된 증명서는 Phase 6 에서 이 표를 참조하지 않고 내용을 복사해 둔다.
--
-- 지키는 것 (화면·domain 규칙과 같은 것을 DB 도 한 번 더 막는다)
--   * 강사 표에는 주민등록번호·주소 칸이 **없다** (결정 D3). 시험이 열 목록을 고정한다.
--   * 강사 이름에 UNIQUE 를 걸지 않는다 — 동명이인. 식별은 id / uuid.
--   * 날짜는 'YYYY-MM-DD' 만 받는다. 없는 날짜(2022-02-30)·'현재' 같은 글자는 거부한다.
--     `date(x) IS x` — date() 는 없는 날짜를 다음 달로 넘기거나 NULL 을 주므로 같지 않다.
--     `=` 가 아니라 `IS` 인 까닭: NULL 과의 `=` 는 NULL 이고 CHECK 는 NULL 을 통과로 본다.
--   * 재직중(ACTIVE) 은 종료일·종료 사유가 없고, 종료(ENDED) 는 둘 다 있으며 종료일 ≥ 시작일.
--   * 지우지 않고 보관한다(archived_at). 빈칸은 NULL 이 아니라 ''.

-- 엑셀 가져오기 묶음. Phase 8 이 열을 더한다(ALTER TABLE ADD COLUMN).
-- 지금 만들어 두는 까닭: SQLite 는 나중에 외래키를 붙일 수 없고, 없는 표를 가리키는
-- 외래키는 INSERT·DELETE 를 막는다(직접 확인). careers.import_id 가 기댈 자리만 둔다.
CREATE TABLE imports (
    id         INTEGER PRIMARY KEY,
    uuid       TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE instructors (
    id            INTEGER PRIMARY KEY,
    uuid          TEXT NOT NULL UNIQUE CHECK (length(uuid) = 36),
    name          TEXT NOT NULL CHECK (name <> '' AND name = trim(name)),
    -- 동명이인 구분 메모 (예: 1985년생, 마술 강사, 이전 계약자)
    distinguisher TEXT NOT NULL DEFAULT '',
    phone         TEXT NOT NULL DEFAULT '',
    memo          TEXT NOT NULL DEFAULT '',
    archived_at   TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
) STRICT;

CREATE INDEX idx_instructors_name ON instructors (name);

CREATE TABLE careers (
    id            INTEGER PRIMARY KEY,
    uuid          TEXT NOT NULL UNIQUE CHECK (length(uuid) = 36),
    instructor_id INTEGER NOT NULL REFERENCES instructors (id),
    program_name  TEXT NOT NULL CHECK (program_name <> '' AND program_name = trim(program_name)),
    position      TEXT NOT NULL CHECK (position <> '' AND position = trim(position)),
    duty          TEXT NOT NULL CHECK (duty <> '' AND duty = trim(duty)),
    start_date    TEXT NOT NULL CHECK (date(start_date) IS start_date),
    end_date      TEXT CHECK (end_date IS NULL OR date(end_date) IS end_date),
    status        TEXT NOT NULL CHECK (status IN ('ACTIVE', 'ENDED')),
    end_reason    TEXT CHECK (end_reason IS NULL OR end_reason IN ('CONTRACT_END', 'TERMINATED')),
    import_id     INTEGER REFERENCES imports (id),
    memo          TEXT NOT NULL DEFAULT '',
    archived_at   TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    CHECK (
        (status = 'ACTIVE' AND end_date IS NULL AND end_reason IS NULL)
        OR
        (status = 'ENDED' AND end_date IS NOT NULL AND end_reason IS NOT NULL AND end_date >= start_date)
    )
) STRICT;

CREATE INDEX idx_careers_instructor ON careers (instructor_id, start_date);
