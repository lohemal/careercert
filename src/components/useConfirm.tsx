import { useCallback, useRef, useState, type ReactNode } from 'react'

import { isAppError } from '@/ipc/invoke'
import { Button, Modal } from './ui'

interface Ask {
  title: string
  message: ReactNode
  confirmLabel?: string
  tone?: 'primary' | 'danger'
}

/**
 * 확인 창. `const [confirm, dialog] = useConfirm()` → `if (await confirm({...})) …` → JSX 에 `{dialog}`.
 * WebView2 에는 `window.confirm` 대신 이것을 쓴다(업무 앱 모양 그대로, 키보드 Esc 로 취소).
 */
export function useConfirm(): [(ask: Ask) => Promise<boolean>, ReactNode] {
  const [ask, setAsk] = useState<Ask | null>(null)
  const resolver = useRef<((ok: boolean) => void) | null>(null)

  const confirm = useCallback((a: Ask) => {
    setAsk(a)
    return new Promise<boolean>((resolve) => {
      resolver.current = resolve
    })
  }, [])

  const close = (ok: boolean) => {
    resolver.current?.(ok)
    resolver.current = null
    setAsk(null)
  }

  const dialog = ask ? (
    <Modal
      title={ask.title}
      onClose={() => close(false)}
      width={460}
      footer={
        <>
          <Button onClick={() => close(false)}>취소</Button>
          <Button variant={ask.tone ?? 'primary'} onClick={() => close(true)} autoFocus>
            {ask.confirmLabel ?? '확인'}
          </Button>
        </>
      }
    >
      <div>{ask.message}</div>
    </Modal>
  ) : null

  return [confirm, dialog]
}

/** 서버가 "미래 종료일 확인이 필요하다" 고 거절한 오류인가 */
export function isFutureEndUnconfirmed(e: unknown): e is { code: string; userMessage: string } {
  return isAppError(e) && e.code === 'FUTURE_END_UNCONFIRMED'
}

/**
 * 미래 종료일 확인 흐름. 먼저 확인 없이 보내 보고, 서버가 `FUTURE_END_UNCONFIRMED` 로 거절하면
 * 그 문구로 확인 창을 띄운 뒤 `confirmFuture = true` 로 다시 보낸다. 취소하면 null.
 *
 * 미래 여부의 판단은 서버(domain)가 한다 — 화면이 날짜를 견주지 않는다.
 */
export async function withFutureConfirm<T>(
  send: (confirmFuture: boolean) => Promise<T>,
  confirm: (ask: Ask) => Promise<boolean>,
): Promise<T | null> {
  try {
    return await send(false)
  } catch (e) {
    if (!isFutureEndUnconfirmed(e)) throw e
    const ok = await confirm({
      title: '종료일 확인',
      message: e.userMessage,
      confirmLabel: '종료일 그대로 저장',
    })
    return ok ? send(true) : null
  }
}
