-- 009_discard_overlap_ack — v0.1.3
--
-- 1) 오발급 폐기 — 잘못 확정한 발급 기록(ISSUED)을 통째로 없애 발급번호를 다시 쓸 수 있게 한다.
--    '발급 취소'(ISSUED → VOIDED, 기록은 남고 번호는 영구 점유)와는 다른 업무다.
--
--    발급 기록의 불변 트리거는 그대로 지키되, **폐기 허가 표(certificate_discard_gate)에 그 발급 번호가
--    들어 있는 동안에만** 그 발급 기록과 거기 딸린 줄(경력 줄·출력 이력)을 지울 수 있게 조건을 단다.
--    허가는 service::issuance::discard 가 한 트랜잭션 안에서 넣고 지운다 — 트랜잭션이 끝나면 표는 비어 있다.
--    허가 없이 하는 UPDATE·DELETE 는 지금까지처럼 모두 거절된다.
--
--      * certificates            : 허가된 ISSUED 행만 DELETE. 그 행을 copied_from 으로 가리키는 다른 발급 기록은
--                                  그 칸만 NULL 로 (다른 칸·doc_hash 는 그대로 — 지문에 copied_from 은 없다)
--      * certificate_items       : 허가된 발급 기록의 줄만 DELETE
--      * certificate_outputs     : 허가된 발급 기록의 이력만 DELETE
--      * certificate_logos       : 폐기 중이고, 더는 어떤 발급 기록도 쓰지 않는 로고만 DELETE
--
-- 2) 경력 기간 겹침 확인 — 담당자가 '정상 경력' 으로 확인한 겹침 한 쌍. 확인 당시 두 경력의 겹침 판단에 쓰는
--    값(uuid · 시작일 · 종료일/재직중)으로 만든 지문을 저장한다. 지금 지문과 같을 때만 '확인 필요' 에서 뺀다.
--    한 쌍에 한 행(다시 확인하면 덮어씀). 이름·프로그램명 같은 값은 저장하지 않는다.

CREATE TABLE certificate_discard_gate (
    certificate_id INTEGER PRIMARY KEY
) STRICT;

-- 허가는 지금 있는 ISSUED 발급 기록에만 (취소된 건은 폐기할 수 없다)
CREATE TRIGGER certificate_discard_gate_issued_only BEFORE INSERT ON certificate_discard_gate
WHEN NOT EXISTS (SELECT 1 FROM certificates WHERE id = NEW.certificate_id AND status = 'ISSUED')
BEGIN
    SELECT RAISE(ABORT, 'only ISSUED certificates can be discarded');
END;

CREATE TRIGGER certificate_discard_gate_no_update BEFORE UPDATE ON certificate_discard_gate
BEGIN
    SELECT RAISE(ABORT, 'discard gate rows cannot be changed');
END;

-- 발급 기록 지우기: 허가된 ISSUED 행만
DROP TRIGGER certificates_no_delete;

CREATE TRIGGER certificates_no_delete BEFORE DELETE ON certificates
WHEN NOT (
        OLD.status = 'ISSUED'
    AND EXISTS (SELECT 1 FROM certificate_discard_gate WHERE certificate_id = OLD.id)
)
BEGIN
    SELECT RAISE(ABORT, 'certificates cannot be deleted');
END;

-- 발급 기록 고치기: ① ISSUED → VOIDED (008 과 같다) ② 폐기되는 발급 기록을 가리키는 copied_from 만 NULL 로
DROP TRIGGER certificates_only_void;

CREATE TRIGGER certificates_only_void BEFORE UPDATE ON certificates
WHEN NOT (
        NEW.id IS OLD.id AND NEW.uuid IS OLD.uuid
    AND NEW.issue_no IS OLD.issue_no AND NEW.issue_no_key IS OLD.issue_no_key
    AND NEW.issued_on IS OLD.issued_on AND NEW.template_version IS OLD.template_version
    AND NEW.title IS OLD.title AND NEW.purpose IS OLD.purpose AND NEW.holder_name IS OLD.holder_name
    AND NEW.sensitive_nonce IS OLD.sensitive_nonce AND NEW.sensitive_cipher IS OLD.sensitive_cipher
    AND NEW.key_id IS OLD.key_id AND NEW.masked_rrn IS OLD.masked_rrn AND NEW.rrn_display IS OLD.rrn_display
    AND NEW.issuer_title IS OLD.issuer_title AND NEW.department IS OLD.department
    AND NEW.manager_name IS OLD.manager_name AND NEW.phone IS OLD.phone
    AND NEW.item_count IS OLD.item_count AND NEW.doc_hash IS OLD.doc_hash
    AND NEW.source_instructor_id IS OLD.source_instructor_id
    AND NEW.created_at IS OLD.created_at AND NEW.created_by IS OLD.created_by
    AND NEW.logo_sha256 IS OLD.logo_sha256
    AND (
            (   OLD.status = 'ISSUED' AND NEW.status = 'VOIDED'
            AND NEW.copied_from_certificate_id IS OLD.copied_from_certificate_id)
         OR (   NEW.status IS OLD.status AND NEW.voided_at IS OLD.voided_at AND NEW.void_reason IS OLD.void_reason
            AND OLD.copied_from_certificate_id IS NOT NULL AND NEW.copied_from_certificate_id IS NULL
            AND EXISTS (SELECT 1 FROM certificate_discard_gate WHERE certificate_id = OLD.copied_from_certificate_id))
    )
)
BEGIN
    SELECT RAISE(ABORT, 'certificates are immutable (only ISSUED -> VOIDED is allowed)');
END;

DROP TRIGGER certificate_items_no_delete;

CREATE TRIGGER certificate_items_no_delete BEFORE DELETE ON certificate_items
WHEN NOT EXISTS (SELECT 1 FROM certificate_discard_gate WHERE certificate_id = OLD.certificate_id)
BEGIN
    SELECT RAISE(ABORT, 'certificate items are immutable');
END;

DROP TRIGGER certificate_outputs_no_delete;

CREATE TRIGGER certificate_outputs_no_delete BEFORE DELETE ON certificate_outputs
WHEN NOT EXISTS (SELECT 1 FROM certificate_discard_gate WHERE certificate_id = OLD.certificate_id)
BEGIN
    SELECT RAISE(ABORT, 'certificate outputs are append-only');
END;

DROP TRIGGER certificate_logos_no_delete;

CREATE TRIGGER certificate_logos_no_delete BEFORE DELETE ON certificate_logos
WHEN NOT (
        EXISTS (SELECT 1 FROM certificate_discard_gate)
    AND NOT EXISTS (SELECT 1 FROM certificates WHERE logo_sha256 = OLD.sha256)
)
BEGIN
    SELECT RAISE(ABORT, 'certificate logos are immutable');
END;

-- 경력 기간 겹침 확인 (한 쌍에 한 행. career_a_uuid < career_b_uuid 로 순서를 정한다)
CREATE TABLE career_overlap_acks (
    id              INTEGER PRIMARY KEY,
    career_a_uuid   TEXT NOT NULL CHECK (length(career_a_uuid) = 36),
    career_b_uuid   TEXT NOT NULL CHECK (length(career_b_uuid) = 36),
    -- SHA-256 16진수 — domain::overlap::fingerprint
    fingerprint     TEXT NOT NULL UNIQUE CHECK (length(fingerprint) = 64),
    acknowledged_at TEXT NOT NULL,
    CHECK (career_a_uuid < career_b_uuid),
    UNIQUE (career_a_uuid, career_b_uuid)
) STRICT;
