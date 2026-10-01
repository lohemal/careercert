-- 008_school_logo — v0.1.2
--
-- 1) school_logo: 지금 학교 로고 (한 줄, 없을 수 있음). 이 프로그램이 만든 PNG 보관본을 DB 에 넣는다.
--    **원본 파일 경로는 저장하지 않는다** — 원본을 옮기거나 지워도 출력이 깨지지 않고, 백업·복원에 함께 간다.
-- 2) certificate_logos: 발급 당시 로고의 보관본. 지문(sha256)으로 한 번만 저장하고 고치거나 지우지 않는다.
-- 3) certificates.logo_sha256: 그 발급본이 쓴 로고 (2판부터, 없으면 NULL). 1판(v0.1.1 까지)은 언제나 NULL.
--    값은 doc_hash 의 정규 표현(`logo=` 줄)에도 들어간다 — 바꾸면 지문 검증에 걸린다.
--
-- 기존 발급본은 손대지 않는다(template_version 1 · logo_sha256 NULL 그대로).

CREATE TABLE school_logo (
    id         INTEGER PRIMARY KEY CHECK (id = 1),
    mime       TEXT NOT NULL CHECK (mime = 'image/png'),
    data       BLOB NOT NULL CHECK (length(data) > 0),
    sha256     TEXT NOT NULL CHECK (length(sha256) = 64),
    width      INTEGER NOT NULL CHECK (width > 0),
    height     INTEGER NOT NULL CHECK (height > 0),
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE certificate_logos (
    sha256     TEXT PRIMARY KEY CHECK (length(sha256) = 64),
    mime       TEXT NOT NULL CHECK (mime = 'image/png'),
    data       BLOB NOT NULL CHECK (length(data) > 0),
    width      INTEGER NOT NULL CHECK (width > 0),
    height     INTEGER NOT NULL CHECK (height > 0),
    created_at TEXT NOT NULL
) STRICT;

CREATE TRIGGER certificate_logos_no_update BEFORE UPDATE ON certificate_logos
BEGIN
    SELECT RAISE(ABORT, 'certificate logos are immutable');
END;

CREATE TRIGGER certificate_logos_no_delete BEFORE DELETE ON certificate_logos
BEGIN
    SELECT RAISE(ABORT, 'certificate logos are immutable');
END;

ALTER TABLE certificates ADD COLUMN logo_sha256 TEXT REFERENCES certificate_logos (sha256)
    CHECK (logo_sha256 IS NULL OR (length(logo_sha256) = 64 AND template_version >= 2));

-- 발급 기록의 불변 트리거에 새 칸을 넣어 다시 만든다 (006 과 같은 조건 + logo_sha256)
DROP TRIGGER certificates_only_void;

CREATE TRIGGER certificates_only_void BEFORE UPDATE ON certificates
WHEN NOT (
        OLD.status = 'ISSUED' AND NEW.status = 'VOIDED'
    AND NEW.id IS OLD.id AND NEW.uuid IS OLD.uuid
    AND NEW.issue_no IS OLD.issue_no AND NEW.issue_no_key IS OLD.issue_no_key
    AND NEW.issued_on IS OLD.issued_on AND NEW.template_version IS OLD.template_version
    AND NEW.title IS OLD.title AND NEW.purpose IS OLD.purpose AND NEW.holder_name IS OLD.holder_name
    AND NEW.sensitive_nonce IS OLD.sensitive_nonce AND NEW.sensitive_cipher IS OLD.sensitive_cipher
    AND NEW.key_id IS OLD.key_id AND NEW.masked_rrn IS OLD.masked_rrn AND NEW.rrn_display IS OLD.rrn_display
    AND NEW.issuer_title IS OLD.issuer_title AND NEW.department IS OLD.department
    AND NEW.manager_name IS OLD.manager_name AND NEW.phone IS OLD.phone
    AND NEW.item_count IS OLD.item_count AND NEW.doc_hash IS OLD.doc_hash
    AND NEW.source_instructor_id IS OLD.source_instructor_id
    AND NEW.copied_from_certificate_id IS OLD.copied_from_certificate_id
    AND NEW.created_at IS OLD.created_at AND NEW.created_by IS OLD.created_by
    AND NEW.logo_sha256 IS OLD.logo_sha256
)
BEGIN
    SELECT RAISE(ABORT, 'certificates are immutable (only ISSUED -> VOIDED is allowed)');
END;
