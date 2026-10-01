/**
 * 숨은 출력 창(print.html)의 스크립트. Rust `print::Engine` 이 부른다.
 *
 *   window.__certLoad(id) — 맡겨 둔 html 을 가져와(print_take, 한 번만) 문서로 그리고, 내장 글꼴을
 *                           다 읽으면(2판은 경력표를 쪽마다 나눈 뒤 — print-split.ts) print_loaded 로 알린다. 그다음 Rust 가 PDF·인쇄를 한다.
 *   window.__certClear()  — 문서를 비운다(출력이 끝나면 언제나). 주민번호·주소가 DOM 에 남지 않게.
 *
 * 여기서는 html 을 어디에도 저장하지 않고, 화면에 찍지도 않는다(창은 보이지 않는다).
 */
import { invoke } from '@tauri-apps/api/core'

import { splitCareerPages } from './print-split'

declare global {
  interface Window {
    __certLoad: (id: number) => Promise<void>
    __certClear: () => void
  }
}

/** 양식 CSS 의 내장 글꼴 이름 — Rust `render` 와 같아야 한다 */
const FONT = 'CertSans'

window.__certLoad = async (id: number) => {
  try {
    const html = await invoke<string | null>('print_take', { id })
    if (html == null) throw new Error('맡겨 둔 문서가 없음')
    // <html> 안쪽 전체(head·body)를 바꾼다. 이 스크립트(모듈)는 계속 살아 있다.
    document.documentElement.innerHTML = html
    // 내장 글꼴을 굵기마다 먼저 읽고, 남은 글꼴 읽기도 끝날 때까지 기다린다
    await Promise.all([document.fonts.load(`400 16px ${FONT}`, '가'), document.fonts.load(`700 16px ${FONT}`, '가')])
    await document.fonts.ready
    const fontOk = document.fonts.check(`400 16px ${FONT}`, '가') && document.fonts.check(`700 16px ${FONT}`, '가')
    // 2판: 실제 글꼴로 줄 높이를 재서 경력표를 쪽마다 한 덩어리로 나눈다(1판은 그대로)
    if (fontOk) splitCareerPages(document)
    // 그림이 다 그려진 다음 프레임에서 알린다
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
    await invoke('print_loaded', { id, ok: true, fontOk, error: null })
  } catch (e) {
    window.__certClear()
    await invoke('print_loaded', { id, ok: false, fontOk: false, error: e instanceof Error ? e.message : '불러오기 실패' })
  }
}

window.__certClear = () => {
  document.documentElement.innerHTML = '<head><meta charset="utf-8"></head><body></body>'
}

void invoke('print_host_ready')
