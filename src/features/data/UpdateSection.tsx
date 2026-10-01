import { useState } from 'react'
import { Download, RefreshCw } from 'lucide-react'

import { Button, Notice } from '@/components/ui'
import { findUpdate, installUpdate } from '@/components/UpdateNotice'
import s from './data.module.css'

type Update = import('@tauri-apps/plugin-updater').Update

/**
 * 프로그램 버전 · [업데이트 확인] (v0.1.1).
 * 자동 확인(사이드바 알림)을 기다리지 않고 바로 확인할 때 쓴다. 연습용·개발판에서는 확인하지 않는다.
 */
export function UpdateSection({ version, sandbox }: { version: string; sandbox: boolean }) {
  const [busy, setBusy] = useState(false)
  const [found, setFound] = useState<Update | null>(null)
  const [message, setMessage] = useState<{ tone: 'info' | 'success' | 'warn'; text: string } | null>(null)
  const disabled = sandbox || import.meta.env.DEV

  const check = async () => {
    setBusy(true)
    setMessage(null)
    try {
      const u = await findUpdate()
      setFound(u)
      setMessage(u ? { tone: 'info', text: `새 버전 ${u.version} 이 있습니다.` } : { tone: 'success', text: '지금 쓰는 버전이 최신입니다.' })
    } catch {
      setMessage({ tone: 'warn', text: '업데이트를 확인하지 못했습니다. 인터넷 연결을 확인해 주세요.' })
    } finally {
      setBusy(false)
    }
  }

  const install = async () => {
    if (!found) return
    setBusy(true)
    const failed = await installUpdate(found, (text) => setMessage({ tone: 'info', text }))
    setMessage({ tone: 'warn', text: failed })
    setBusy(false)
  }

  return (
    <section className={s.section}>
      <h3 className={s.title}>프로그램</h3>
      <p className={s.help}>
        지금 버전 <b>v{version}</b>
        {disabled && ' — 연습용·개발판은 업데이트를 확인하지 않습니다.'}
      </p>
      {message && <Notice tone={message.tone}>{message.text}</Notice>}
      <div className={s.actions}>
        <Button icon={RefreshCw} onClick={check} disabled={busy || disabled}>
          업데이트 확인
        </Button>
        {found && (
          <Button variant="primary" icon={Download} onClick={install} disabled={busy}>
            v{found.version} 설치
          </Button>
        )}
      </div>
    </section>
  )
}
