-- 002_school_settings — Phase 1
--
-- 학교 기본정보. 학교가 하나이므로 행도 하나(id = 1)뿐이다.
-- 증명서를 발급할 때 이 값들을 **복사**해 발급 기록에 넣는다(Phase 6) — 여기를 고쳐도
-- 이미 발급한 증명서는 바뀌지 않는다.
--
--   * 비어 있음은 NULL 이 아니라 '' 로 둔다. "아직 안 적음" 을 한 가지 모양으로만 다룬다.
--   * 학교명(school_name)과 발급자 표기(issuer_title)는 **따로** 저장한다.
--     발급자 표기를 학교명에서 만들어 내지 않는다 — 사람이 고친 값을 지키기 위해서다.
--   * 기본값 두 개는 Rust `domain::settings::{DEFAULT_TITLE, DEFAULT_PURPOSE}` 와 같아야 한다
--     (시험이 확인한다).

CREATE TABLE school_settings (
    id                INTEGER PRIMARY KEY CHECK (id = 1),
    school_name       TEXT NOT NULL DEFAULT '',
    certificate_title TEXT NOT NULL DEFAULT '방과후학교 개인위탁 외부강사 활동 확인서',
    issuer_title      TEXT NOT NULL DEFAULT '',
    department        TEXT NOT NULL DEFAULT '',
    manager_name      TEXT NOT NULL DEFAULT '',
    phone             TEXT NOT NULL DEFAULT '',
    default_purpose   TEXT NOT NULL DEFAULT '기관제출',
    -- 한 번도 저장하지 않았으면 NULL
    updated_at        TEXT
) STRICT;

INSERT INTO school_settings (id) VALUES (1);
