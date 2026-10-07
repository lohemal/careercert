import { useState } from 'react'
import { Ban, CopyPlus, FileDown, FilePlus2, Printer, Trash2 } from 'lucide-react'

import { Badge, Button, Card, ErrorNotice, Notice } from '@/components/ui'
import { useConfirm } from '@/components/useConfirm'
import { issuanceApi, type Issued, type OutputRecord } from '@/ipc/issuance'
import { DiscardDialog } from './DiscardDialog'
import { PrintDialog } from './PrintDialog'
import { VoidDialog } from './VoidDialog'
import s from './IssuedPanel.module.css'

interface Props {
  issued: Issued
  onChange: (issued: Issued) => void
  /** 'issued' = 방금 발급 확정함(작성 화면) · 'history' = 발급이력 상세 */
  variant?: 'issued' | 'history'
  /** 빈 작성 화면으로 */
  onNew?: () => void
  /** 이 내용으로 새 증명서 작성 (재출력과 다르다 — 새 발급 기록을 만든다) */
  onCopy?: () => void
  /** 원본 발급 기록 보기 */
  onOpen?: (id: number) => void
  /** 오발급 폐기가 끝났다 — 이 기록은 더 없다 */
  onDiscarded?: () => void
}

/** `2026-10-01T09:05:00` → `2026.10.01. 09:05` */
export function stamp(iso: string) {
  const [d, t = ''] = iso.split('T')
  return `${d.replace(/-/g, '.')}. ${t.slice(0, 5)}`
}

export function rrnDisplayLabel(code: string) {
  return code === 'MASK_BACK' ? '뒷자리 가림' : '전체 표시'
}

const FAIL_TEXT: Record<string, string> = {
  PrinterUnavailable: '프린터를 쓸 수 없음',
  OtherError: '인쇄 오류',
}

/**
 * 발급 기록 — 발급 당시 스냅샷 보기 · 재출력(인쇄·PDF 저장) · 발급 취소 · 오발급 폐기 · 이 내용으로 새 증명서 작성.
 *
 * * 보이는 값은 모두 발급 기록에서 왔다(복호화하지 않음). 주민번호는 가린 값만, 주소는 보이지 않는다.
 * * 재출력은 발급 번호만 보낸다 — Rust 가 발급 기록에서 다시 만들고 지문을 검증한다(지금 원장·설정을 읽지 않음).
 * * 취소된 건은 출력·취소·폐기 버튼이 없다(서버도 거절한다).
 * * 발급 취소(효력 취소, 기록·번호 남음)와 오발급 폐기(잘못 확정한 기록 삭제, 번호 재사용)는 나란히 차이를 적어 보인다.
 */
export function IssuedPanel({ issued, onChange, variant = 'issued', onNew, onCopy, onOpen, onDiscarded }: Props) {
  const [confirm, confirmDialog] = useConfirm()
  const [printing, setPrinting] = useState(false)
  const [voiding, setVoiding] = useState(false)
  const [discarding, setDiscarding] = useState(false)
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
          <br />교부하지 않은 증명서를 잘못 확정한 것이라면 '발급 취소' 대신 '오발급 폐기' 를 쓰세요.
        </>
      ),
    })
    if (ok) setVoiding(true)
  }

  const title = voided ? '취소된 발급 건' : variant === 'issued' ? '발급되었습니다' : `발급 기록 · ${issued.issueNo}`

  return (
    <Card
      title={title}
      description={voided ? undefined : '발급 당시 내용 그대로 정식 출력합니다. 발급번호·발급일·내용은 바뀌지 않습니다.'}
    >
      {voided ? (
        <Notice tone="error">{issued.message ?? '취소된 발급 건입니다. 정식 출력할 수 없습니다.'}</Notice>
      ) : (
        variant === 'issued' && (
          <Notice tone="success">
            <span className={s.head}>
              {issued.issueNo} · {issued.holderName} 증명서를 발급했습니다 ({stamp(issued.createdAt)}).
            </span>
          </Notice>
        )
      )}

      <dl className={s.facts}>
        <dt>발급번호</dt>
        <dd className={s.strong}>{issued.issueNo}</dd>
        <dt>발급일</dt>
        <dd>{issued.issuedOnLabel}</dd>
        <dt>상태</dt>
        <dd>
          {voided ? (
            <>
              <Badge tone="error">취소</Badge>{' '}
              <span className={s.sub}>
                {issued.voidedAt && stamp(issued.voidedAt)} · 사유: {issued.voidReason}
              </span>
            </>
          ) : (
            <>
              <Badge tone="success">발급</Badge> <span className={s.sub}>{stamp(issued.createdAt)} 확정</span>
            </>
          )}
        </dd>
        <dt>성명</dt>
        <dd>{issued.holderName}</dd>
        <dt>주민등록번호</dt>
        <dd>
          {issued.maskedRrn} <span className={s.sub}>증명서에 {rrnDisplayLabel(issued.rrnDisplay)}</span>
        </dd>
        <dt>용도</dt>
        <dd>{issued.purpose}</dd>
        <dt>학교장 표기</dt>
        <dd>{issued.issuerTitle}</dd>
        <dt>담당</dt>
        <dd>
          {issued.department} · {issued.managerName} · {issued.phone}
        </dd>
        {issued.copiedFrom && (
          <>
            <dt>참고한 발급</dt>
            <dd>
              {onOpen ? (
                <button type="button" className={s.link} onClick={() => onOpen(issued.copiedFrom!.id)}>
                  {issued.copiedFrom.issueNo}
                </button>
              ) : (
                issued.copiedFrom.issueNo
              )}{' '}
              <span className={s.sub}>의 내용으로 새로 작성</span>
            </dd>
          </>
        )}
      </dl>

      <div>
        <h3 className={s.histTitle}>경력 {issued.items.length}건 (발급 당시)</h3>
        <table className={s.items}>
          <thead>
            <tr>
              <th>부터</th>
              <th>까지</th>
              <th>프로그램명</th>
              <th>직위</th>
              <th>지도사항</th>
            </tr>
          </thead>
          <tbody>
            {issued.items.map((it) => (
              <tr key={it.seq}>
                <td>{it.fromText}</td>
                <td>{it.toText}</td>
                <td>{it.programName}</td>
                <td>{it.position}</td>
                <td>{it.duty}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

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
          </>
        )}
        <span className={s.spacer} />
        {onCopy && (
          <Button icon={CopyPlus} onClick={onCopy} title="이 발급 기록을 참고해 새 증명서를 작성합니다. 재출력이 아닙니다.">
            이 내용으로 새 증명서 작성
          </Button>
        )}
        {onNew && (
          <Button icon={FilePlus2} onClick={onNew}>
            새 증명서 작성
          </Button>
        )}
      </div>

      {!voided && (
        <div className={s.danger}>
          <div className={s.dangerItem}>
            <Button variant="ghost" icon={Ban} onClick={startVoid}>
              발급 취소
            </Button>
            <p className={s.dangerText}>
              정상적으로 발급했던 증명서의 효력을 취소합니다. 기록은 남고, 발급번호는 다시 사용할 수 없습니다.
            </p>
          </div>
          {onDiscarded && (
            <div className={s.dangerItem}>
              <Button variant="ghost" icon={Trash2} onClick={() => setDiscarding(true)}>
                오발급 폐기
              </Button>
              <p className={s.dangerText}>
                잘못 확정한 증명서 기록을 폐기합니다. 폐기 후 이 발급번호는 다시 사용할 수 있습니다.
              </p>
            </div>
          )}
        </div>
      )}

      <OutputHistory outputs={issued.outputs} />

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
      {discarding && onDiscarded && (
        <DiscardDialog
          issued={issued}
          onClose={() => setDiscarding(false)}
          onDiscarded={() => {
            setDiscarding(false)
            onDiscarded()
          }}
        />
      )}
      {confirmDialog}
    </Card>
  )
}

function OutputHistory({ outputs }: { outputs: OutputRecord[] }) {
  if (outputs.length === 0) return <p className={s.muted}>아직 정식 출력한 적이 없습니다.</p>
  return (
    <div>
      <h3 className={s.histTitle}>정식 출력 이력 (시간순)</h3>
      <ul className={s.hist}>
        {outputs.map((o, i) => (
          <li key={i}>
            <span className={s.sub}>{stamp(o.at)}</span>
            <Badge tone={o.result === 'SUCCESS' ? 'success' : 'error'}>{o.result === 'SUCCESS' ? '성공' : '실패'}</Badge>
            <span>{o.outputType === 'PDF' ? 'PDF 저장' : `인쇄 · ${o.printerName} · ${o.copies}부`}</span>
            {o.detail && (
              <span className={s.sub}>
                까닭: {FAIL_TEXT[o.detail] ?? o.detail} ({o.detail})
              </span>
            )}
          </li>
        ))}
      </ul>
      <p className={s.muted}>인쇄 '성공'은 프린터 대기열로 정상 전송되었다는 뜻입니다. 저장한 파일의 위치는 기록하지 않습니다.</p>
    </div>
  )
}
