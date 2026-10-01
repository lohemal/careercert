import { useEffect, useState } from 'react'
import { Download } from 'lucide-react'

import { invoke } from '@/ipc/invoke'
import s from './UpdateNotice.module.css'

/**
 * 새 버전 알림 (사이드바 아래).
 *
 * * 시작을 막지 않는다 — 화면이 뜬 뒤 조용히 한 번 확인하고, 없거나 실패하면 아무것도 보이지 않는다.
 * * **연습용·개발 빌드에서는 확인하지 않는다** — 개발판이 실제 설치 파일을 받아 덮어쓰면 안 된다.
 * * 사람이 눌러야 설치한다. 내려받기와 **서명 확인**은 Tauri 업데이터가 한다(앱에 든 공개 키).
 * * 설치 직전에 자료 연결을 닫는다(WAL 을 비우고 파일을 놓음) — 설치 프로그램이 앱을 끝낼 때 자료가 열린 채가 아니게.
 * * `relaunch()` 를 부르지 않는다 — Windows 에서 install() 은 설치 프로그램을 띄우고 앱을 스스로 끝내며,
 *   설치가 끝나면 설치 프로그램이 앱을 다시 켠다. 여기서 다시 켜면 설치와 경쟁해 옛 버전이 남는다.
 * * 새 버전 첫 실행에서 자료 구조가 바뀌면 기존 마이그레이션이 **직전에 백업**(before_migration)한 뒤 올린다.
 */
type Phase =
  | { kind: 'quiet' }
  | { kind: 'found'; version: string }
  | { kind: 'working'; message: string }
  | { kind: 'failed'; message: string }

const DELAY_MS = 3000

export function UpdateNotice({ enabled }: { enabled: boolean }) {
  const [phase, setPhase] = useState<Phase>({ kind: 'quiet' })
  const [update, setUpdate] = useState<import('@tauri-apps/plugin-updater').Update | null>(null)

  useEffect(() => {
    if (!enabled) return
    let alive = true
    const timer = setTimeout(async () => {
      try {
        const { check } = await import('@tauri-apps/plugin-updater')
        const found = await check()
        if (!alive || !found) return
        setUpdate(found)
        setPhase({ kind: 'found', version: found.version })
      } catch {
        // 인터넷이 없거나 확인에 실패해도 아무 말 하지 않는다 — 발급 업무와 상관없다
      }
    }, DELAY_MS)
    return () => {
      alive = false
      clearTimeout(timer)
    }
  }, [enabled])

  if (phase.kind === 'quiet' || !update) return null

  const install = async () => {
    let total = 0
    let got = 0
    setPhase({ kind: 'working', message: '새 버전을 내려받는 중…' })
    try {
      await update.download((e) => {
        if (e.event === 'Started') total = e.data.contentLength ?? 0
        if (e.event === 'Progress') {
          got += e.data.chunkLength
          if (total > 0) setPhase({ kind: 'working', message: `내려받는 중… ${Math.round((got / total) * 100)}%` })
        }
      })
    } catch {
      setPhase({ kind: 'failed', message: '새 버전을 내려받지 못했습니다. 지금 버전은 그대로 쓸 수 있습니다.' })
      return
    }
    setPhase({ kind: 'working', message: '설치합니다. 프로그램이 잠시 닫혔다가 다시 켜집니다…' })
    try {
      await invoke('app_prepare_update')
      await update.install()
      // 여기로 돌아오지 않는 것이 정상이다 (설치 프로그램이 앱을 끝낸다)
    } catch {
      setPhase({ kind: 'failed', message: '설치하지 못했습니다. 프로그램을 닫았다가 다시 열어 주세요. 자료는 그대로입니다.' })
    }
  }

  return (
    <div className={s.box} role="status">
      {phase.kind === 'found' && (
        <>
          <span>
            새 버전 <b>{phase.version}</b> 이 나왔습니다.
          </span>
          <div className={s.actions}>
            <button type="button" className={s.primary} onClick={install}>
              <Download size={14} /> 업데이트
            </button>
            <button type="button" className={s.ghost} onClick={() => setPhase({ kind: 'quiet' })}>
              나중에
            </button>
          </div>
        </>
      )}
      {phase.kind === 'working' && <span>{phase.message}</span>}
      {phase.kind === 'failed' && (
        <>
          <span>{phase.message}</span>
          <button type="button" className={s.ghost} onClick={() => setPhase({ kind: 'quiet' })}>
            닫기
          </button>
        </>
      )}
    </div>
  )
}
