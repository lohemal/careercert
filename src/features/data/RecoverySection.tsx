import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { KeyRound } from 'lucide-react'

import { Button, ErrorNotice, Field, Input, Modal, Notice } from '@/components/ui'
import { RECOVERY_MAX, RECOVERY_MIN, recoveryApi } from '@/ipc/data'
import { historyApi } from '@/ipc/issuance'
import s from './data.module.css'

export const RECOVERY_NOT_SET_WARNING =
  '복구 비밀번호가 설정되지 않았습니다. PC 고장·교체 또는 Windows 계정 변경 시 기존 발급 기록의 개인정보를 복구하지 못할 수 있습니다.'

const CANNOT_RECOVER =
  '복구 비밀번호는 프로그램에서도 확인하거나 되찾을 수 없습니다. 잊어버리면 다른 PC 또는 다른 Windows 계정에서 암호화된 발급 개인정보를 복구할 수 없습니다.'

/** 글자 수 (앞뒤 공백 포함 — 프로그램이 지우지 않는다) */
function len(v: string) {
  return [...v].length
}

/**
 * 복구 비밀번호 — 상태 안내와 설정·변경.
 * 비밀번호는 이 창의 state 에만 잠깐 있고 닫으면 비운다. 서버에 직접 보낸다(캐시 없음).
 */
export function RecoverySection() {
  const recovery = useQuery({ queryKey: ['recovery'], queryFn: historyApi.recovery, staleTime: 0 })
  const [dialog, setDialog] = useState<'set' | 'change' | null>(null)
  const [done, setDone] = useState<string | null>(null)
  const r = recovery.data

  return (
    <section className={s.section}>
      <h3 className={s.title}>복구 비밀번호</h3>
      <ErrorNotice error={recovery.error} />
      {r && !r.ready && r.certificates > 0 && <Notice tone="warn">{RECOVERY_NOT_SET_WARNING}</Notice>}
      {r && (
        <Notice tone={r.ready ? 'success' : r.certificates > 0 ? 'warn' : 'info'}>
          <b>암호화 자료 복구 설정: {r.ready ? '준비됨' : '설정되지 않음'}</b>
          <br />
          {r.message}
        </Notice>
      )}
      <p className={s.help}>
        복구 비밀번호는 PC 를 바꾸거나 Windows 계정이 바뀌었을 때 발급 기록의 주민등록번호·주소를 다시 열고, 이동용 백업을 만들고 복원하는 데 씁니다.
        같은 PC 에서 평소에 발급·출력할 때는 묻지 않습니다.
      </p>
      {done && <Notice tone="success">{done}</Notice>}
      <div className={s.actions}>
        {r && !r.ready && (
          <Button variant="primary" icon={KeyRound} onClick={() => setDialog('set')}>
            복구 비밀번호 설정
          </Button>
        )}
        {r?.ready && (
          <Button icon={KeyRound} onClick={() => setDialog('change')}>
            복구 비밀번호 변경
          </Button>
        )}
      </div>
      {dialog && (
        <PasswordDialog
          mode={dialog}
          onClose={() => setDialog(null)}
          onDone={(text) => {
            setDialog(null)
            setDone(text)
          }}
        />
      )}
    </section>
  )
}

function PasswordDialog({ mode, onClose, onDone }: { mode: 'set' | 'change'; onClose: () => void; onDone: (text: string) => void }) {
  const qc = useQueryClient()
  const [old, setOld] = useState('')
  const [pw, setPw] = useState('')
  const [confirm, setConfirm] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const n = len(pw)
  const tooShort = n < RECOVERY_MIN
  const mismatch = confirm.length > 0 && pw !== confirm
  const ready = !tooShort && n <= RECOVERY_MAX && pw === confirm && (mode === 'set' || old.length > 0)

  const run = async () => {
    setBusy(true)
    setError(null)
    try {
      if (mode === 'set') await recoveryApi.set(pw, confirm)
      else await recoveryApi.change(old, pw, confirm)
      void qc.invalidateQueries({ queryKey: ['recovery'] })
      onDone(mode === 'set' ? '복구 비밀번호를 설정했습니다.' : '복구 비밀번호를 바꿨습니다. 이전 비밀번호는 더 이상 쓰지 않습니다.')
    } catch (e) {
      setError(e)
    } finally {
      setOld('')
      setBusy(false)
    }
  }

  return (
    <Modal
      title={mode === 'set' ? '복구 비밀번호 설정' : '복구 비밀번호 변경'}
      onClose={onClose}
      busy={busy}
      width={520}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            닫기
          </Button>
          <Button variant="primary" onClick={run} disabled={busy || !ready}>
            {busy ? '확인 중…' : mode === 'set' ? '설정' : '변경'}
          </Button>
        </>
      }
    >
      <Notice tone="warn">{CANNOT_RECOVER}</Notice>
      <p className={s.help}>
        {RECOVERY_MIN}자 이상이면 됩니다. 문자 종류를 섞을 필요는 없으니 <b>기억하기 쉬운 긴 문장</b>을 권합니다(예: 띄어쓰기를 포함한 짧은 문장).
        입력한 그대로가 비밀번호입니다 — 앞뒤 공백도 지우지 않습니다.
      </p>
      {mode === 'change' && (
        <Field label="지금 복구 비밀번호" hint="지금 비밀번호가 맞아야 바꿀 수 있습니다.">
          <Input type="password" autoComplete="off" value={old} onChange={(e) => setOld(e.target.value)} />
        </Field>
      )}
      <Field label={mode === 'set' ? '복구 비밀번호' : '새 복구 비밀번호'} hint={`${n}자${tooShort && n > 0 ? ` — ${RECOVERY_MIN}자 이상 필요` : ''}`}>
        <Input type="password" autoComplete="new-password" value={pw} onChange={(e) => setPw(e.target.value)} />
      </Field>
      <Field label="비밀번호 확인" hint={mismatch ? '위와 같지 않습니다.' : undefined}>
        <Input type="password" autoComplete="new-password" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
      </Field>
      <ErrorNotice error={error} />
    </Modal>
  )
}
