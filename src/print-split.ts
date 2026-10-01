/**
 * 증명서 2판의 경력표를 **쪽마다 한 덩어리**로 나눈다(print-host.ts 가 글꼴을 다 읽은 뒤 부른다).
 *
 * 왜: 2판은 왼쪽 큰 칸의 '경력 사항' 글자와 학교 로고를 경력 영역 위에 겹쳐 그린다(Rust `render::v2`).
 * 경력표가 여러 쪽에 걸치면 그 겹친 그림은 표 전체에 한 번만 그려진다(쪽마다 다시 그리지 않는다 —
 * WebView2 는 쪽이 갈라진 상자의 배경·겹친 그림을 되풀이하지 않는다). 그래서 출력 전에 실제 글꼴·쪽 크기로
 * 줄 높이를 재서, 한 쪽에 들어가는 줄만큼씩 경력표를 덩어리로 나눈다. 덩어리마다 머리글·글자·로고가 있고
 * 두 번째 덩어리부터는 새 쪽에서 시작한다(CSS `.career-zone + .career-zone`).
 *
 * 나누는 기준은 원래 쪽 나눔(줄은 쪼개지지 않는다)과 같다 — 쪽에 들어가는 줄 수가 1판과 같다.
 * 마지막 덩어리(용도·학교장)는 그대로 흐른다. 경력 줄이 아주 적은 덩어리에는 로고를 넣지 않는다.
 * 1판 문서(`data-split` 없음)는 건드리지 않는다.
 */

/** A4 쪽에서 여백(위 16mm · 아래 15mm · 좌우 14mm)을 뺀 내용 영역 — Rust `page_rule` 과 같아야 한다 */
const CONTENT_W_MM = 210 - 28
const CONTENT_H_MM = 297 - 16 - 15
/** 로고를 넣을 최소 경력 줄 영역 높이(약 2줄) — 이보다 작으면 로고가 얼룩처럼 작아 보인다 */
const MIN_LOGO_AREA_PT = 80
/** 재는 값의 반올림 차이로 줄이 쪽을 넘지 않게 두는 여유 */
const EPS_PX = 1

const PX_PER_MM = 96 / 25.4
const PX_PER_PT = 96 / 72

export interface PageInput {
  /** 첫 쪽에서 경력표가 쓸 수 있는 높이(경력표 시작 위치부터 쪽 끝까지) */
  firstRoom: number
  /** 다음 쪽부터 경력표가 쓸 수 있는 높이 */
  nextRoom: number
  /** 머리글(기간·프로그램명…) 높이 — 덩어리마다 하나 */
  headH: number
  /** 경력 줄마다 위·아래 위치(한 표로 그렸을 때) */
  rowTop: number[]
  rowBottom: number[]
}

/**
 * 줄을 쪽별로 나눈다(줄은 쪼개지 않는다). 첫 쪽은 경력표 시작 위치부터, 다음 쪽부터는 쪽 맨 위부터.
 * 첫 쪽에 한 줄도 안 들어가면 경력표 전체가 다음 쪽에서 시작한다(원래 쪽 나눔과 같게).
 * 한 쪽보다 높은 줄은 그 쪽에 혼자 둔다.
 */
export function planPages({ firstRoom, nextRoom, headH, rowTop, rowBottom }: PageInput): {
  chunks: number[][]
  startOnNextPage: boolean
} {
  const pages: number[][] = []
  let room = firstRoom
  let cur: number[] = []
  for (let i = 0; i < rowTop.length; i++) {
    const need = headH + rowBottom[i] - rowTop[cur.length ? cur[0] : i]
    if (need > room && (cur.length || pages.length === 0)) {
      pages.push(cur)
      cur = []
      room = nextRoom
    }
    cur.push(i)
  }
  pages.push(cur)
  return { chunks: pages.filter((p) => p.length), startOnNextPage: pages[0].length === 0 }
}

export function splitCareerPages(doc: Document): void {
  const zone = doc.querySelector<HTMLElement>('.career-zone[data-split="page"]')
  const sheet = doc.querySelector<HTMLElement>('.sheet')
  if (!zone || !sheet) return
  const root = doc.documentElement
  const keepWidth = root.style.width
  // 인쇄할 때의 내용 영역 너비로 잰다(창 너비와 다르다)
  root.style.width = `${CONTENT_W_MM}mm`
  try {
    const pageH = CONTENT_H_MM * PX_PER_MM
    const style = getComputedStyle(sheet)
    // 양식 테두리는 쪽마다 위·아래로 닫는다(box-decoration-break: clone)
    const frame = parseFloat(style.borderTopWidth) + parseFloat(style.borderBottomWidth)
    const top = (el: Element) => el.getBoundingClientRect().top - root.getBoundingClientRect().top
    const table = zone.querySelector('table')
    const body = table?.tBodies[0]
    if (!table?.tHead || !body) return
    const headH = table.tHead.getBoundingClientRect().height
    const rows = Array.from(body.rows)
    const rowTop = rows.map(top)
    const rowBottom = rows.map((r) => r.getBoundingClientRect().bottom - root.getBoundingClientRect().top)

    const { chunks, startOnNextPage } = planPages({
      firstRoom: pageH - top(zone) - parseFloat(style.borderBottomWidth) - EPS_PX,
      nextRoom: pageH - frame - EPS_PX,
      headH,
      rowTop,
      rowBottom,
    })

    const zones: HTMLElement[] = []
    chunks.forEach((idx, k) => {
      const z = k === 0 ? zone : (zone.cloneNode(true) as HTMLElement)
      z.querySelector('tbody')!.replaceChildren(...idx.map((i) => rows[i]))
      const area = rowBottom[idx[idx.length - 1]] - rowTop[idx[0]]
      if (area < MIN_LOGO_AREA_PT * PX_PER_PT) z.querySelector('.logo-mark')?.remove()
      if (k > 0) zones[k - 1].after(z)
      zones.push(z)
    })
    if (startOnNextPage) zone.style.breakBefore = 'page'
    for (const z of zones) z.removeAttribute('data-split')
  } finally {
    root.style.width = keepWidth
  }
}
