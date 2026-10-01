import { invoke as tauriInvoke } from '@tauri-apps/api/core'

/** Rust `AppError`와 1:1 대응 */
export interface AppError {
  code: string
  userMessage: string
  detail?: string | null
}

export function isAppError(e: unknown): e is AppError {
  return (
    typeof e === 'object' &&
    e !== null &&
    'code' in e &&
    'userMessage' in e &&
    typeof (e as AppError).userMessage === 'string'
  )
}

/** 예상하지 못한 오류의 기본 안내 */
export const UNEXPECTED = '작업을 완료하지 못했습니다. 같은 문제가 계속되면 프로그램을 다시 시작해 주세요.'

/** 개발 중에만 원문을 보인다 — 설치본은 오류 코드만(원문에 입력값·경로가 섞일 수 있다) */
const DEV = import.meta.env.DEV

/** 화면에 그대로 보여줄 수 있는 문장을 뽑아낸다. Rust 문장은 이미 정리되어 온다. */
export function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.userMessage
  return UNEXPECTED
}

export function errorDetail(e: unknown): string | undefined {
  if (isAppError(e)) return e.detail ?? undefined
  if (DEV && e instanceof Error) return e.stack
  return '오류 코드: UNKNOWN'
}

/**
 * Tauri 명령 호출 래퍼.
 * Rust 쪽 오류는 항상 AppError 형태로 정규화해서 던진다.
 */
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args)
  } catch (e) {
    if (isAppError(e)) throw e
    // Tauri 가 직접 낸 오류(인자 형식 등)는 원문에 입력값이 들어 있을 수 있다 — 설치본에서는 보이지 않는다
    const err: AppError = {
      code: 'UNKNOWN',
      userMessage: UNEXPECTED,
      detail: DEV ? (typeof e === 'string' ? e : JSON.stringify(e)) : '오류 코드: UNKNOWN',
    }
    throw err
  }
}
