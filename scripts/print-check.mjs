#!/usr/bin/env node
// 실제 PDF 출력 확인 (개발용). 앱을 띄운 상태에서 돌린다 — 가상 자료로만 그린다.
//
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9225 npm run app:sandbox
//   npm run check:print            (다른 창에서)
//
// 확인하는 것: 경력 1·5·6·15·40건 × (발급 전 미리보기·발급본)
//   * PDF 를 만들 수 있다 · 쪽 크기가 A4 · 실린 글꼴이 Noto Sans KR Regular·Bold 둘뿐
//   * 쪽 수 (1·1·1·2·3)
//   * 기본 파일 이름에 주민번호·주소가 없다
// 쪽 수·크기·글꼴은 Rust `print::inspect` 가 읽는다. 워터마크·주민번호 표시는 Rust 시험(render_tests)이 본다.

const PORT = Number(process.env.CDP_PORT || 9225)
const EXPECT = { 1: 1, 5: 1, 6: 1, 15: 2, 40: 3 }

const list = await (await fetch(`http://127.0.0.1:${PORT}/json`).catch(() => {
  console.error(`[print-check] 앱에 붙지 못했습니다(포트 ${PORT}). 위 안내대로 앱을 먼저 띄워 주세요.`)
  process.exit(2)
})).json()
const page = list.find((t) => t.type === 'page' && !t.url.includes('print.html'))
const ws = new WebSocket(page.webSocketDebuggerUrl)
await new Promise((r) => (ws.onopen = r))
let seq = 0
const call = (args) =>
  new Promise((resolve) => {
    const id = ++seq
    const expression = `window.__TAURI_INTERNALS__.invoke('dev_print_spike', ${JSON.stringify(args)})
      .then((b) => JSON.parse(new TextDecoder().decode(b))).catch((e) => ({ error: e }))`
    ws.addEventListener('message', function on(m) {
      const d = JSON.parse(m.data)
      if (d.id !== id) return
      ws.removeEventListener('message', on)
      resolve(d.result?.result?.value)
    })
    ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
  })

let failed = 0
const check = (ok, what) => {
  console.log(`${ok ? '  ok ' : '  NG '} ${what}`)
  if (!ok) failed++
}

for (const mode of ['draft', 'issued']) {
  for (const [n, pages] of Object.entries(EXPECT)) {
    const r = await call({ kind: 'render-facts', rows: Number(n), mode })
    if (r?.error) {
      check(false, `${mode} ${n}건: ${JSON.stringify(r.error)}`)
      continue
    }
    const f = r.facts
    const a4 = f.pageSizesMm.length === f.pages && f.pageSizesMm.every(([w, h]) => Math.abs(w - 210) <= 0.5 && Math.abs(h - 297) <= 0.5)
    const fonts = JSON.stringify(f.fonts) === JSON.stringify(['NotoSansKR-Bold', 'NotoSansKR-Regular'])
    check(f.pages === pages && a4 && fonts, `${mode} ${n}건 → ${f.pages}쪽(기대 ${pages}) · A4 ${a4} · 글꼴 ${f.fonts.join(',')} · ${f.bytes}B`)
    check(!/\d{6}-?\d{7}|시험로/.test(r.fileName), `파일 이름 ${r.fileName}`)
  }
}
ws.close()
console.log(failed ? `[print-check] ${failed}건 실패` : '[print-check] 모두 통과')
process.exit(failed ? 1 : 0)
