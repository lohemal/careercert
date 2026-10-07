import { useState } from 'react'

import { Button, ErrorNotice, Field, Modal, Textarea } from '@/components/ui'
import { issuanceApi, type Issued } from '@/ipc/issuance'

interface Props {
  issued: Issued
  onClose: () => void
  onVoided: (issued: Issued) => void
}

/** 발급 취소 — 사유 필수. 내용은 그대로 남고 정식 출력만 막힌다. 발급번호는 다시 쓸 수 없다(오발급 폐기와 다르다). */
export function VoidDialog({ issued, onClose, onVoided }: Props) {
  const [reason, setReason] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<unknown>(null)

  const run = async () => {
    setBusy(true)
    setError(null)
    try {
      onVoided(await issuanceApi.void(issued.id, reason))
    } catch (e) {
      setError(e)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title={`발급 취소 — ${issued.issueNo}`}
      onClose={onClose}
      busy={busy}
      width={480}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            닫기
          </Button>
          <Button variant="danger" onClick={run} disabled={busy || !reason.trim()}>
            {busy ? '취소 중…' : '발급 취소'}
          </Button>
        </>
      }
    >
      <Field label="취소 사유" hint="예: 교부한 증명서의 효력 취소 요청, 교부 뒤 내용 오류 발견. 사유는 발급 기록에 남습니다(200자 이내). 교부하지 않은 증명서를 잘못 확정했다면 오발급 폐기를 쓰세요.">
        <Textarea value={reason} rows={3} maxLength={200} autoFocus onChange={(e) => setReason(e.target.value)} />
      </Field>
      <ErrorNotice error={error} />
    </Modal>
  )
}
