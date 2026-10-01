-- 006_certificates — Phase 6
--
-- 발급 기록. **고칠 수 없는 스냅샷**이다. 지금 강사·경력·학교 설정을 다시 읽지 않고도
-- 발급 당시 증명서를 똑같이 다시 그릴 수 있도록, 증명서에 들어간 값을 모두 **복사**해 둔다.
--
--   * 주민번호·주소는 평문으로 두지 않는다 — AES-256-GCM 암호문(sensitive_*)만. 목록용 가린 값만 평문.
--   * 데이터 키는 key_store 에 DPAPI 로 감싼 모양으로만 있다.
--   * 발급번호는 **상태와 상관없이** 한 번 쓰면 다시 쓰지 않는다(취소된 번호도). issue_no 는 입력 원문,
--     issue_no_key 는 공백을 모두 뺀 비교용 값 — 둘 다 UNIQUE.
--   * 허용되는 변경은 ISSUED → VOIDED (status · voided_at · void_reason) 하나뿐. 나머지 UPDATE·DELETE 는
--     트리거가 거부한다. 경력 줄(certificate_items)은 넣은 뒤 고칠 수도 지울 수도 없다.
--   * 출력 이력(certificate_outputs)은 덧붙이기만. 개인정보·저장 경로를 담는 칸이 없다.

CREATE TABLE key_store (
    id          INTEGER PRIMARY KEY,
    key_id      TEXT NOT NULL UNIQUE CHECK (length(key_id) = 36),
    algorithm   TEXT NOT NULL CHECK (algorithm = 'AES-256-GCM'),
    -- Windows DPAPI(현재 사용자)로 감싼 32바이트 키
    dpapi_blob  BLOB NOT NULL,
    -- 복구 비밀번호로 감싼 사본 (Phase 8)
    recovery_blob BLOB,
    created_at  TEXT NOT NULL
) STRICT;

CREATE TRIGGER key_store_no_delete BEFORE DELETE ON key_store
BEGIN
    SELECT RAISE(ABORT, 'key_store rows cannot be deleted');
END;

CREATE TABLE certificates (
    id                 INTEGER PRIMARY KEY,
    uuid               TEXT NOT NULL UNIQUE CHECK (length(uuid) = 36),
    -- 발급번호: 앞뒤 공백만 뺀 입력 원문 (출력에 이것을 쓴다)
    issue_no           TEXT NOT NULL UNIQUE CHECK (issue_no <> '' AND issue_no = trim(issue_no)),
    -- 중복 비교용: 공백을 모두 뺀 값. '제2026-152호' 와 '제 2026-152 호' 를 같은 번호로 본다
    issue_no_key       TEXT NOT NULL UNIQUE CHECK (issue_no_key <> ''),
    issued_on          TEXT NOT NULL CHECK (date(issued_on) IS issued_on),
    template_version   INTEGER NOT NULL CHECK (template_version >= 1),
    title              TEXT NOT NULL CHECK (title <> ''),
    purpose            TEXT NOT NULL CHECK (purpose <> ''),
    holder_name        TEXT NOT NULL CHECK (holder_name <> ''),
    -- 주민번호·주소 (AES-256-GCM). AAD = uuid · key_id
    sensitive_nonce    BLOB NOT NULL CHECK (length(sensitive_nonce) = 12),
    sensitive_cipher   BLOB NOT NULL,
    key_id             TEXT NOT NULL REFERENCES key_store (key_id),
    -- 목록 표시용 가린 값 'YYMMDD-N******'
    masked_rrn         TEXT NOT NULL CHECK (length(masked_rrn) = 14 AND substr(masked_rrn, 9) = '******'),
    rrn_display        TEXT NOT NULL CHECK (rrn_display IN ('FULL', 'MASK_BACK')),
    issuer_title       TEXT NOT NULL,
    department         TEXT NOT NULL,
    manager_name       TEXT NOT NULL,
    phone              TEXT NOT NULL,
    -- 경력 줄 수 (넣을 수 있는 줄의 상한 — 발급 뒤 줄을 더 붙이지 못하게)
    item_count         INTEGER NOT NULL CHECK (item_count >= 1),
    -- HMAC-SHA256(데이터 키에서 나눈 지문 키, 정규화한 문서) 16진수
    doc_hash           TEXT NOT NULL CHECK (length(doc_hash) = 64),
    status             TEXT NOT NULL DEFAULT 'ISSUED' CHECK (status IN ('ISSUED', 'VOIDED')),
    voided_at          TEXT,
    void_reason        TEXT,
    -- 추적용 (출력에는 쓰지 않는다)
    source_instructor_id      INTEGER REFERENCES instructors (id),
    copied_from_certificate_id INTEGER REFERENCES certificates (id),
    created_at         TEXT NOT NULL,
    created_by         TEXT NOT NULL,
    CHECK (
        (status = 'ISSUED' AND voided_at IS NULL AND void_reason IS NULL)
        OR
        (status = 'VOIDED' AND voided_at IS NOT NULL AND void_reason IS NOT NULL AND trim(void_reason) <> '')
    )
) STRICT;

CREATE INDEX idx_certificates_issued_on ON certificates (issued_on);
CREATE INDEX idx_certificates_holder ON certificates (holder_name);

-- 새 발급은 언제나 ISSUED 로 들어온다
CREATE TRIGGER certificates_insert_issued BEFORE INSERT ON certificates
WHEN NEW.status <> 'ISSUED'
BEGIN
    SELECT RAISE(ABORT, 'certificates must be inserted as ISSUED');
END;

CREATE TRIGGER certificates_no_delete BEFORE DELETE ON certificates
BEGIN
    SELECT RAISE(ABORT, 'certificates cannot be deleted');
END;

-- 허용: ISSUED → VOIDED, 그리고 status · voided_at · void_reason 만 바뀐다
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
)
BEGIN
    SELECT RAISE(ABORT, 'certificates are immutable (only ISSUED -> VOIDED is allowed)');
END;

CREATE TABLE certificate_items (
    id                INTEGER PRIMARY KEY,
    certificate_id    INTEGER NOT NULL REFERENCES certificates (id),
    seq               INTEGER NOT NULL CHECK (seq >= 1),
    -- 추적용 (지금 경력이 바뀌어도 이 줄은 그대로)
    source_career_id  INTEGER REFERENCES careers (id),
    start_date        TEXT NOT NULL CHECK (date(start_date) IS start_date),
    -- 발급일 기준 끝. NULL = 증명서에 '현재'
    end_date          TEXT CHECK (end_date IS NULL OR date(end_date) IS end_date),
    from_text         TEXT NOT NULL,
    to_text           TEXT NOT NULL,
    program_name      TEXT NOT NULL,
    position          TEXT NOT NULL,
    duty              TEXT NOT NULL,
    UNIQUE (certificate_id, seq)
) STRICT;

-- 줄은 발급할 때 정한 개수까지만 넣을 수 있다 (발급 뒤에 줄을 더 붙이지 못한다)
CREATE TRIGGER certificate_items_limit BEFORE INSERT ON certificate_items
WHEN (SELECT COUNT(*) FROM certificate_items WHERE certificate_id = NEW.certificate_id)
     >= (SELECT item_count FROM certificates WHERE id = NEW.certificate_id)
  OR NEW.seq > (SELECT item_count FROM certificates WHERE id = NEW.certificate_id)
  OR (SELECT status FROM certificates WHERE id = NEW.certificate_id) <> 'ISSUED'
BEGIN
    SELECT RAISE(ABORT, 'certificate items are fixed at issuance');
END;

CREATE TRIGGER certificate_items_no_update BEFORE UPDATE ON certificate_items
BEGIN
    SELECT RAISE(ABORT, 'certificate items are immutable');
END;

CREATE TRIGGER certificate_items_no_delete BEFORE DELETE ON certificate_items
BEGIN
    SELECT RAISE(ABORT, 'certificate items are immutable');
END;

-- 정식 출력 이력. 덧붙이기만.
CREATE TABLE certificate_outputs (
    id              INTEGER PRIMARY KEY,
    certificate_id  INTEGER NOT NULL REFERENCES certificates (id),
    output_type     TEXT NOT NULL CHECK (output_type IN ('PRINT', 'PDF')),
    result          TEXT NOT NULL CHECK (result IN ('SUCCESS', 'FAILED')),
    printer_name    TEXT,
    copies          INTEGER CHECK (copies IS NULL OR copies BETWEEN 1 AND 20),
    -- 실패 까닭 코드 등 (개인정보·경로 금지)
    detail          TEXT NOT NULL DEFAULT '' CHECK (length(detail) <= 200),
    at              TEXT NOT NULL,
    CHECK (
        (output_type = 'PRINT' AND printer_name IS NOT NULL AND copies IS NOT NULL)
        OR
        (output_type = 'PDF' AND printer_name IS NULL AND copies IS NULL)
    )
) STRICT;

CREATE INDEX idx_certificate_outputs ON certificate_outputs (certificate_id);

CREATE TRIGGER certificate_outputs_no_update BEFORE UPDATE ON certificate_outputs
BEGIN
    SELECT RAISE(ABORT, 'certificate outputs are append-only');
END;

CREATE TRIGGER certificate_outputs_no_delete BEFORE DELETE ON certificate_outputs
BEGIN
    SELECT RAISE(ABORT, 'certificate outputs are append-only');
END;
