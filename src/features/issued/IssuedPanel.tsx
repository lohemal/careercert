import { useState } from 'react'
import { Ban, FilePlus2, FileDown, Printer } from 'lucide-react'

import { Badge, Button, Card, ErrorNotice, Notice } from '@/components/ui'
import { useConfirm } from '@/components/useConfirm'
import { issuanceApi, type Issued } from '@/ipc/issuance'
import { PrintDialog } from './PrintDialog'
import { VoidDialog } from './VoidDialog'
import s from './IssuedPanel.module.css'

interface Props {
  issued: Issued
  onChange: (issued: Issued) => void
  /** 새 증명서 작성으로 */
  onNew?: () => void
}

/** `2026-10-01T09:05:00` → `2026.10.01. 09:05` */
function stamp(iso: string) {
  const [d, t = ''] = iso.split('T')
  return `${d.replace(/-/g, '.')}. ${t.slice(0, 5)}`
}

/**
 * 발급된 증명서 — 정식 PDF 저장·인쇄·발급 취소.
 * 정식 출력은 발급 번호만 보낸다. 내용은 Rust 가 발급 기록에서 다시 만들고 검증한다.
 * 취소된 건은 출력 버튼이 없다(서버도 거절한다).
 */
export function IssuedPanel({ issued, onChange, onNew }: Props) {
  const [confirm, confirmDialog] = useConfirm()
  const [printing, setPrinting] = useState(false)
  const [voiding, setVoiding] = useState(false)
  const [saving, setSaving] = useState(false)
  const [notice, setNotice] = useState<{ tone: 'success' | 'warn' | 'error'; text: string } | null>(null)
  const [error, setError] = useState<unknown>(null)
  const voided = issued.status === 'VOIDED'

  const savePdf = async () => {
    setSaving(true)
    setError(null)
    setNotice(null)
    try {
      const r = await issuanceApi.savePdf(issued.id)
      onChange(r.issued)
      if (r.saved) setNotice({ tone: 'success', text: `PDF 를 저장했습니다: ${r.fileName}` })
    } catch (e) {
      setError(e)
      // 실패도 이력에 남았을 수 있으니 다시 읽는다
      issuanceApi.get(issued.id).then(onChange, () => undefined)
    } finally {
      setSaving(false)
    }
  }

  const startVoid = async () => {
    const ok = await confirm({
      title: '발급 취소',
      tone: 'danger',
      confirmLabel: '취소 사유 입력',
      message: (
        <>
          {issued.issueNo} 증명서의 발급을 취소합니다. 내용은 지워지지 않고 남지만 다시 정식 출력할 수 없습니다.
          <br />이 발급번호는 다시 쓸 수 없습니다. 고쳐서 다시 발급하려면 NEIS 에서 새 번호를 받아야 합니다.
        </>
      ),
    })
    if (ok) setVoiding(true)
  }

  return (
    <Card
      title={voided ? '취소된 발급 건' : '발급되었습니다'}
      description={voided ? undefined : '발급 기록을 그대로 정식 출력합니다. 발급번호·발급일은 바뀌지 않습니다.'}
    >
      {voided ? (
        <Notice tone="error">{issued.message ?? '취소된 발급 건입니다. 정식 출력할 수 없습니다.'}</Notice>
      ) : (
        <Notice tone="success">
          <span className={s.head}>
            {issued.issueNo} · {issued.holderName} 증명서를 발급했습니다 ({stamp(issued.createdAt)}).
          </span>
        </Notice>
      )}
      <dl className={s.facts}>
        <dt>발급번호</dt>
        <dd className={s.strong}>{issued.issueNo}</dd>
        <dt>성명</dt>
        <dd>{issued.holderName}</dd>
        <dt>주민등록번호</dt>
        <dd>
          {issued.maskedRrn} <span className={s.sub}>{issued.rrnDisplay === 'MASK_BACK' ? '증명서에 뒷자리 가림' : '증명서에 전체 표시'}</span>
        </dd>
        <dt>발급일 · 용도</dt>
        <dd>
          {issued.issuedOnLabel} · {issued.purpose}
        </dd>
        <dt>경력</dt>
        <dd>{issued.itemCount}건</dd>
        <dt>상태</dt>
        <dd>
          {voided ? (
            <>
              <Badge tone="error">발급 취소</Badge> <span className={s.sub}>{issued.voidedAt && stamp(issued.voidedAt)} · 사유: {issued.voidReason}</span>
            </>
          ) : (
            <Badge tone="success">발급</Badge>
          )}
        </dd>
      </dl>

      {notice && <Notice tone={notice.tone}>{notice.text}</Notice>}
      <ErrorNotice error={error} />

      <div className={s.actions}>
        {!voided && (
          <>
            <Button variant="primary" icon={Printer} onClick={() => setPrinting(true)}>
              인쇄
            </Button>
            <Button icon={FileDown} onClick={savePdf} disabled={saving}>
              {saving ? 'PDF 만드는 중…' : 'PDF 저장'}
            </Button>
            <span className={s.spacer} />
            <Button variant="ghost" icon={Ban} onClick={startVoid}>
              발급 취소
            </Button>
          </>
        )}
        {voided && <span className={s.spacer} />}
        {onNew && (
          <Button icon={FilePlus2} onClick={onNew}>
            새 증명서 작성
          </Button>
        )}
      </div>

      <OutputHistory issued={issued} />

      {printing && (
        <PrintDialog
          issued={issued}
          onClose={() => setPrinting(false)}
          onDone={(r, text, tone) => {
            onChange(r)
            setNotice({ tone, text })
          }}
        />
      )}
      {voiding && (
        <VoidDialog
          issued={issued}
          onClose={() => setVoiding(false)}
          onVoided={(r) => {
            setVoiding(false)
            onChange(r)
            setNotice({ tone: 'warn', text: '발급을 취소했습니다. 이 증명서는 더 이상 정식 출력할 수 없습니다.' })
          }}
        />
      )}
      {confirmDialog}
    </Card>
  )
}

function OutputHistory({ issued }: { issued: Issued }) {
  if (issued.outputs.length === 0) return <p className={s.muted}>아직 정식 출력한 적이 없습니다.</p>
  return (
    <div>
      <h3 className={s.histTitle}>정식 출력 이력</h3>
      <ul className={s.hist}>
        {issued.outputs.map((o, i) => (
          <li key={i}>
            <Badge tone={o.result === 'SUCCESS' ? 'success' : 'error'}>{o.result === 'SUCCESS' ? '성공' : '실패'}</Badge>
            <span>{o.outputType === 'PDF' ? 'PDF 저장' : `인쇄 · ${o.printerName} · ${o.copies}부`}</span>
            <span className={s.sub}>{stamp(o.at)}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}
