import { useState } from 'react'
import { AlertOctagon, BadgeCheck } from 'lucide-react'

import { Button, ErrorNotice, Modal } from '@/components/ui'
import type { PrepareRequest, Prepared } from '@/ipc/certificate'
import { isAppError } from '@/ipc/invoke'
import { issuanceApi, type Issued } from '@/ipc/issuance'
import s from './ConfirmIssueDialog.module.css'

interface Props {
  request: PrepareRequest
  prepared: Prepared
  onIssued: (issued: Issued) => void
  /** 내용 확인 뒤 자료가 바뀌어 거절되었다 — 내용 확인을 다시 해야 한다 */
  onStale: (message: string) => void
  onClose: () => void
}

/**
 * 발급 확정 창 — 무엇을 발급하는지 다시 보여 주고, 확정이 필요한 경고는 하나하나 체크받는다.
 * 경고가 없으면 체크 칸도 없다. 서버는 같은 요청으로 문서를 다시 만들어 검증한다(바뀌었으면 거절).
 * 요청에 주민번호·주소가 있으므로 useMutation 을 쓰지 않고 직접 부른다.
 */
export function ConfirmIssueDialog({ request, prepared, onIssued, onStale, onClose }: Props) {
  const d = prepared.doc
  const [acked, setAcked] = useState<Set<string>>(new Set())
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const all = prepared.warnings.every((w) => acked.has(w.ackKey))
  // 주민번호 생년월일 이상을 맨 위에 — 다른 경고보다 눈에 띄게
  const warnings = [...prepared.warnings].sort((a, b) => Number(b.code === 'RRN_BIRTH_DATE') - Number(a.code === 'RRN_BIRTH_DATE'))

  const toggle = (k: string) => {
    const next = new Set(acked)
    if (next.has(k)) next.delete(k)
    else next.add(k)
    setAcked(next)
  }

  const run = async () => {
    setBusy(true)
    setError(null)
    try {
      const issued = await issuanceApi.issue({
        prepare: request,
        reviewToken: prepared.reviewToken,
        acknowledged: [...acked],
        copiedFrom: null,
      })
      onIssued(issued)
    } catch (e) {
      if (isAppError(e) && e.code === 'CERT_CHANGED') {
        onStale(e.userMessage)
        return
      }
      setError(e)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title="발급 확정"
      onClose={onClose}
      busy={busy}
      width={560}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            취소
          </Button>
          <Button variant="primary" icon={BadgeCheck} onClick={run} disabled={busy || !all}>
            {busy ? '발급 중…' : '발급 확정'}
          </Button>
        </>
      }
    >
      <p className={s.lead}>아래 내용으로 발급합니다. 발급한 뒤에는 고칠 수 없고, 잘못되었으면 발급을 취소하고 새 번호로 다시 발급해야 합니다.</p>
      <dl className={s.facts}>
        <dt>발급번호</dt>
        <dd className={s.strong}>{d.issueNo}</dd>
        <dt>성명</dt>
        <dd className={s.strong}>{d.holderName}</dd>
        <dt>발급일</dt>
        <dd>{d.issuedOnLabel}</dd>
        <dt>용도</dt>
        <dd>{d.purpose}</dd>
        <dt>경력</dt>
        <dd>{d.items.length}건</dd>
        <dt>주민번호 표시</dt>
        <dd>{d.rrnMasked ? `뒷자리 가림 (${d.holderRrnShown})` : '전체 표시'}</dd>
        <dt>학교장 표기</dt>
        <dd>{d.issuerTitle}</dd>
      </dl>

      {warnings.length > 0 && (
        <section className={s.warnings}>
          <h3 className={s.wTitle}>확인이 필요한 경고 — 하나씩 확인해야 발급할 수 있습니다</h3>
          {warnings.map((w) => {
            const rrn = w.code === 'RRN_BIRTH_DATE'
            return (
              <label key={w.ackKey} className={rrn ? s.wItemDanger : s.wItem}>
                <input type="checkbox" checked={acked.has(w.ackKey)} onChange={() => toggle(w.ackKey)} />
                <span>
                  {rrn && (
                    <b className={s.dangerHead}>
                      <AlertOctagon size={14} /> 주민등록번호 확인
                    </b>
                  )}
                  {w.message}
                  <span className={s.ack}>확인했습니다</span>
                </span>
              </label>
            )
          })}
        </section>
      )}
      <ErrorNotice error={error} />
    </Modal>
  )
}
