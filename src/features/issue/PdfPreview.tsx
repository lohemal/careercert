import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import * as pdfjs from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

import s from './PdfPreview.module.css'

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl

interface Props {
  /** WebView2 PrintToPdf 가 만든 PDF 그대로 — 화면용으로 다시 그리지 않는다(쪽 나뉨이 실제와 같다) */
  pdf: ArrayBuffer
  onClose: () => void
}

/**
 * 실제 출력 미리보기. PDF 쪽을 pdf.js 로 그림(canvas)으로만 보여 준다.
 *
 * WebView2 내장 PDF 뷰어를 쓰지 않는 까닭: 저장·인쇄 버튼을 숨기는 설정(HiddenPdfToolbarItems)이
 * 이 런타임에서 듣지 않았다(Phase 5 기술 검증). 발급 전 문서는 저장·인쇄할 수 없어야 하므로
 * 버튼이 아예 없는 그림으로 보여 준다. 오른쪽 클릭(그림 저장)도 막는다.
 * 닫으면 PDF 문서를 버리고 그림을 지운다 — 파일로 남지 않는다.
 */
export function PdfPreview({ pdf, onClose }: Props) {
  const box = useRef<HTMLDivElement>(null)
  const [pages, setPages] = useState(0)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    let task: pdfjs.PDFDocumentLoadingTask | null = null
    const canvases: HTMLCanvasElement[] = []
    ;(async () => {
      try {
        task = pdfjs.getDocument({ data: new Uint8Array(pdf) })
        const doc = await task.promise
        if (cancelled) return
        setPages(doc.numPages)
        const ratio = window.devicePixelRatio || 1
        for (let n = 1; n <= doc.numPages; n++) {
          const page = await doc.getPage(n)
          if (cancelled) return
          const base = page.getViewport({ scale: 1 })
          const cssWidth = Math.min(820, (box.current?.clientWidth ?? 820) - 48)
          const viewport = page.getViewport({ scale: (cssWidth / base.width) * ratio })
          const canvas = document.createElement('canvas')
          canvas.width = Math.floor(viewport.width)
          canvas.height = Math.floor(viewport.height)
          canvas.style.width = `${Math.floor(viewport.width / ratio)}px`
          canvas.className = s.page
          canvas.setAttribute('aria-label', `${n}쪽`)
          box.current?.appendChild(canvas)
          canvases.push(canvas)
          await page.render({ canvas, viewport }).promise
        }
      } catch {
        if (!cancelled) setError('미리보기를 그리지 못했습니다.')
      }
    })()
    return () => {
      cancelled = true
      for (const c of canvases) {
        c.width = 0
        c.height = 0
        c.remove()
      }
      void task?.destroy()
    }
  }, [pdf])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  return (
    <div className={s.scrim} role="dialog" aria-modal="true" aria-label="실제 출력 미리보기" onContextMenu={(e) => e.preventDefault()}>
      <header className={s.head}>
        <span className={s.badge}>발급 전 미리보기</span>
        <span className={s.info}>
          {pages > 0 ? `A4 · ${pages}쪽` : '그리는 중…'} — 실제 출력과 같은 PDF 입니다. 인쇄·PDF 저장은 발급 확정 후에 할 수 있습니다.
        </span>
        <span className={s.spacer} />
        <button type="button" className={s.close} onClick={onClose}>
          <X size={16} /> 닫기
        </button>
      </header>
      {error && <p className={s.error}>{error}</p>}
      <div className={s.pages} ref={box} />
    </div>
  )
}
