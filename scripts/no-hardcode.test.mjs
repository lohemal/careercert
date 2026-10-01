// 코드에 실제 학교명·담당자·전화번호가 적혀 있지 않은지 본다 (설계안 §2·§6, Phase 1).
// `npm run test:scripts`
//
// 학교 정보는 설정 화면에서만 들어온다. 예시가 필요하면 `○○초등학교`, `000-0000-0000` 처럼 쓴다.
// 문서(docs/)는 기존 양식 분석 내용을 담고 있어 보지 않는다 — 대상은 프로그램 코드다.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(fileURLToPath(new URL('.', import.meta.url)), '..')

/** 프로그램 코드가 있는 곳 */
const TARGETS = ['src', 'src-tauri/src', 'src-tauri/migrations', 'index.html', 'src-tauri/tauri.conf.json', 'src-tauri/tauri.sandbox.conf.json']
const EXT = /\.(ts|tsx|css|rs|sql|html|json)$/

function walk(p, out) {
  const st = statSync(p)
  if (st.isDirectory()) {
    for (const name of readdirSync(p)) walk(join(p, name), out)
  } else if (EXT.test(p)) {
    out.push(p)
  }
  return out
}

const files = TARGETS.flatMap((t) => walk(join(ROOT, t), []))

// 한글 이름 바로 뒤에 붙은 학교 종류 — `○○초등학교` 는 앞이 한글이 아니라 걸리지 않는다
const SCHOOL = /[가-힣]{1,20}(초등학교|중학교|고등학교|초교)/
// 지역번호·휴대전화 모양. `000-0000-0000` 같은 자리표시는 0 으로만 되어 있어 걸리지 않는다
const PHONE = /(?<![0-9])0(?:1[016789]|2|[3-6][1-5]|70)-[1-9][0-9]{2,3}-[0-9]{4}(?![0-9])/

test('검사할 코드 파일이 있다', () => {
  assert.ok(files.length > 10, `파일 ${files.length}개`)
})

for (const [kind, hit] of [
  ['실제 학교명이', (line) => SCHOOL.test(line)],
  ['실제 전화번호가', (line) => PHONE.test(line)],
]) {
  test(`코드에 ${kind} 없다`, () => {
    const found = []
    for (const f of files) {
      readFileSync(f, 'utf8')
        .split(/\r?\n/)
        .forEach((line, i) => {
          if (hit(line)) found.push(`${relative(ROOT, f)}:${i + 1}`)
        })
    }
    assert.deepEqual(found, [])
  })
}

test('검사 규칙 자체가 동작한다', () => {
  assert.ok(SCHOOL.test('가나' + '초등학교'))
  assert.ok(!SCHOOL.test('○○초등학교'))
  assert.ok(!SCHOOL.test('지원 범위: 초등학교'))
  assert.ok(PHONE.test('02-' + '345-6789'))
  assert.ok(PHONE.test('010-' + '1234-5678'))
  assert.ok(!PHONE.test('000-0000-0000'))
  assert.ok(!PHONE.test('2026-10-01'))
})
