import { useEffect, useState } from 'react'
import { Printer, RefreshCw } from 'lucide-react'

import { Badge, Button, ErrorNotice, Field, Modal, Select } from '@/components/ui'
import { useConfirm } from '@/components/useConfirm'
import { errorMessage } from '@/ipc/invoke'
import { issuanceApi, type Issued, type Printers } from '@/ipc/issuance'
import s from './IssuedPanel.module.css'

interface Props {
  issued: Issued
  onClose: () => void
  onDone: (issued: Issued, text: string, tone: 'success' | 'error') => void
}

const STATUS_TEXT: Record<string, string> = {
  SUCCEEDED: '인쇄를 프린터로 보냈습니다.',
  PRINTER_UNAVAILABLE: '프린터를 쓸 수 없어 인쇄하지 못했습니다. 프린터 연결·전원을 확인해 주세요.',
  OTHER_ERROR: '인쇄하지 못했습니다.',
}

/**
 * 정식 인쇄 — 앱 안의 프린터 고르기(Windows 인쇄 대화상자는 쓰지 않는다).
 * 기본 프린터를 미리 고르고, 매수는 1부부터. 보내기 직전에 "○○ 프린터로 1부 인쇄합니다." 를 확인받는다.
 */
export function PrintDialog({ issued, onClose, onDone }: Props) {
  const [confirm, confirmDialog] = useConfirm()
  const [printers, setPrinters] = useState<Printers | null>(null)
  const [loadError, setLoadError] = useState<unknown>(null)
  const [printer, setPrinter] = useState('')
  const [copies, setCopies] = useState(1)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<unknown>(null)

  const load = async () => {
    setLoadError(null)
    try {
      const p = await issuanceApi.printers()
      setPrinters(p)
      setPrinter((cur) => (cur && p.names.includes(cur) ? cur : p.default ?? p.names[0] ?? ''))
    } catch (e) {
      setLoadError(e)
    }
  }
  useEffect(() => {
    void load()
  }, [])

  const info = printers?.list.find((p) => p.name === printer)

  const run = async () => {
    const ok = await confirm({
      title: '인쇄 확인',
      confirmLabel: `${copies}부 인쇄`,
      message: (
        <>
          <b>{printer}</b> 프린터로 <b>{copies}부</b> 인쇄합니다.
          {info && !info.ready && <p className={s.warnLine}>프린터 상태: {info.status}</p>}
        </>
      ),
    })
    if (!ok) return
    setBusy(true)
    setError(null)
    try {
      const r = await issuanceApi.print(issued.id, printer, copies)
      const success = r.status === 'SUCCEEDED'
      onDone(r.issued, success ? `${printer} 프린터로 ${copies}부 인쇄를 보냈습니다.` : STATUS_TEXT[r.status] ?? STATUS_TEXT.OTHER_ERROR, success ? 'success' : 'error')
      onClose()
    } catch (e) {
      setError(e)
      issuanceApi.get(issued.id).then((x) => onDone(x, `인쇄하지 못했습니다. ${errorMessage(e)}`, 'error'), () => undefined)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title="증명서 인쇄"
      onClose={onClose}
      busy={busy}
      width={520}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            닫기
          </Button>
          <Button variant="primary" icon={Printer} onClick={run} disabled={busy || !printer}>
            {busy ? '보내는 중…' : '인쇄'}
          </Button>
        </>
      }
    >
      <p className={s.sub}>
        {issued.issueNo} · {issued.holderName} — A4 세로, 실제 크기(배율 100%)로 인쇄합니다.
      </p>
      <ErrorNotice error={loadError} />
      {printers && printers.names.length === 0 && <p className={s.muted}>설치된 프린터가 없습니다.</p>}
      {printers && printers.names.length > 0 && (
        <>
          <Field label="프린터">
            <Select value={printer} onChange={(e) => setPrinter(e.target.value)}>
              {printers.list.map((p) => (
                <option key={p.name} value={p.name}>
                  {p.name}
                  {p.name === printers.default ? ' (기본)' : ''} — {p.status}
                </option>
              ))}
            </Select>
          </Field>
          {info && (
            <p className={s.statusLine}>
              상태: <Badge tone={info.ready ? 'success' : 'warn'}>{info.status}</Badge>
              {info.jobs > 0 && <span className={s.sub}>대기 중인 작업 {info.jobs}건</span>}
              <Button size="sm" variant="ghost" icon={RefreshCw} onClick={load}>
                다시 확인
              </Button>
            </p>
          )}
          <Field label="매수">
            <Select value={copies} onChange={(e) => setCopies(Number(e.target.value))}>
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={n}>
                  {n}부
                </option>
              ))}
            </Select>
          </Field>
        </>
      )}
      <ErrorNotice error={error} />
      {confirmDialog}
    </Modal>
  )
}
