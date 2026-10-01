// 증명서 2판 경력표 쪽 나누기 시험 (`npm run test:ui`). 높이만 쓰는 순수 계산.

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { planPages } from './print-split.ts'

/** 높이 h 인 줄 n 개를 한 표로 그렸을 때의 위·아래 위치 */
function rows(n: number, h = 48, start = 300) {
  const rowTop = Array.from({ length: n }, (_, i) => start + i * h)
  return { rowTop, rowBottom: rowTop.map((t) => t + h) }
}

const base = { firstRoom: 48 * 15 + 60, nextRoom: 48 * 19 + 60, headH: 60 }

test('한 쪽에 다 들어가면 한 덩어리', () => {
  const p = planPages({ ...base, ...rows(5) })
  assert.deepEqual(p, { chunks: [[0, 1, 2, 3, 4]], startOnNextPage: false })
})

test('첫 쪽에 15줄, 다음 쪽부터 19줄씩 — 줄은 쪼개지 않는다', () => {
  const p = planPages({ ...base, ...rows(40) })
  assert.deepEqual(
    p.chunks.map((c) => c.length),
    [15, 19, 6],
  )
  assert.deepEqual(p.chunks.flat(), Array.from({ length: 40 }, (_, i) => i), '순서 그대로 빠짐없이')
  assert.equal(p.startOnNextPage, false)
})

test('딱 맞으면 그 쪽에 둔다', () => {
  assert.deepEqual(
    planPages({ ...base, ...rows(15) }).chunks.map((c) => c.length),
    [15],
  )
  assert.deepEqual(
    planPages({ ...base, ...rows(16) }).chunks.map((c) => c.length),
    [15, 1],
  )
})

test('줄 높이가 달라도 실제 높이로 나눈다', () => {
  const rowTop = [0, 48, 200, 248]
  const rowBottom = [48, 200, 248, 296]
  const p = planPages({ firstRoom: 60 + 200, nextRoom: 1000, headH: 60, rowTop, rowBottom })
  assert.deepEqual(p.chunks, [[0, 1], [2, 3]])
})

test('첫 쪽에 한 줄도 안 들어가면 다음 쪽에서 시작한다', () => {
  const p = planPages({ ...base, firstRoom: 80, ...rows(5) })
  assert.deepEqual(p, { chunks: [[0, 1, 2, 3, 4]], startOnNextPage: true })
})

test('한 쪽보다 높은 줄은 혼자 둔다(빈 덩어리를 만들지 않는다)', () => {
  const p = planPages({ firstRoom: 500, nextRoom: 500, headH: 60, rowTop: [0, 48, 1048], rowBottom: [48, 1048, 1096] })
  assert.deepEqual(p.chunks, [[0], [1], [2]])
  assert.ok(p.chunks.every((c) => c.length > 0))
})
