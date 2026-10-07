import { useState } from 'react'

import { Button, ErrorNotice, Field, Input, Modal, Notice, Textarea } from '@/components/ui'
import { useConfirm } from '@/components/useConfirm'
import { issuanceApi, type Issued } from '@/ipc/issuance'
import s from './IssuedPanel.module.css'

interface Props {
  issued: Issued
  onClose: () => void
  onDiscarded: () => void
}

export const DISCARD_REASON_MAX = 200

/**
 * 오발급 폐기 — 잘못 확정한 발급 기록을 지우고 발급번호를 다시 쓸 수 있게 한다. (발급 취소와 다르다)
 *
 * 경고 → 폐기 사유(필수, 저장하지 않음) → 발급번호 직접 재입력 → (정식 출력한 적이 있으면) 추가 확인 → 최종 확인.
 * 서버(`service::issuance::discard`)도 같은 것을 다시 검사한다.
 */
export function DiscardDialog({ issued, onClose, onDiscarded }: Props) {
  const [confirm, confirmDialog] = useConfirm()
  const [reason, setReason] = useState('')
  const [typed, setTyped] = useState('')
  const [outputsAck, setOutputsAck] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const printed = issued.outputs.filter((o) => o.result === 'SUCCESS').length
  // 발급번호 원문 규칙과 같게 — 앞뒤 공백만 정리하고 그대로 비교
  const matches = typed.trim() === issued.issueNo
  const ready = !!reason.trim() && matches && (printed === 0 || outputsAck)

  const run = async () => {
    const ok = await confirm({
      title: '오발급 폐기 — 최종 확인',
      tone: 'danger',
      confirmLabel: '폐기',
      message: (
        <>
          <b>{issued.issueNo}</b> 증명서 기록을 발급이력에서 폐기합니다. 되돌릴 수 없습니다.
          <br />폐기 후 이 발급번호로 새 증명서를 발급할 수 있습니다.
        </>
      ),
    })
    if (!ok) return
    setBusy(true)
    setError(null)
    try {
      await issuanceApi.discard(issued.id, { reason, issueNoConfirm: typed, outputsAcknowledged: outputsAck })
      onDiscarded()
    } catch (e) {
      setError(e)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title={`오발급 폐기 — ${issued.issueNo}`}
      onClose={onClose}
      busy={busy}
      width={540}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            닫기
          </Button>
          <Button variant="danger" onClick={run} disabled={busy || !ready}>
            {busy ? '폐기 중…' : '오발급 폐기'}
          </Button>
        </>
      }
    >
      <Notice tone="error">
        이 작업은 잘못 확정한 증명서를 발급 기록에서 폐기합니다. 폐기 후 이 발급번호는 다시 사용할 수 있습니다.
        <br />
        이미 상대방에게 정상적으로 교부한 증명서를 취소하려는 경우에는 <b>'발급 취소'</b>를 사용해야 합니다.
      </Notice>
      {printed > 0 && (
        <Notice tone="warn">
          <b>이 증명서는 이미 {printed}회 인쇄 또는 PDF 저장된 기록이 있습니다.</b>
          <br />
          외부에 정상적으로 교부한 증명서의 효력을 취소하려는 경우에는 '발급 취소'를 사용하세요.
          <label className={s.agree}>
            <input type="checkbox" checked={outputsAck} onChange={(e) => setOutputsAck(e.target.checked)} /> 출력한 증명서를
            외부에 교부하지 않았고, 잘못 확정한 기록이라 폐기합니다.
          </label>
        </Notice>
      )}
      <Field
        label="폐기 사유"
        hint={`예: 발급번호 오입력, 경력 선택 오류, 내용 입력 오류 (${DISCARD_REASON_MAX}자 이내). 확인용으로만 쓰고 저장하지 않습니다.`}
      >
        <Textarea value={reason} rows={2} maxLength={DISCARD_REASON_MAX} autoFocus onChange={(e) => setReason(e.target.value)} />
      </Field>
      <Field label={`확인을 위해 발급번호 '${issued.issueNo}' 를 그대로 입력하세요`}>
        <Input value={typed} autoComplete="off" onChange={(e) => setTyped(e.target.value)} />
      </Field>
      {typed.trim() !== '' && !matches && <p className={s.mismatch}>발급번호가 다릅니다.</p>}
      <ErrorNotice error={error} />
      {confirmDialog}
    </Modal>
  )
}
