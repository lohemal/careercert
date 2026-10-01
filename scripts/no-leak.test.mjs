// 개인정보가 샐 수 있는 길이 코드에 없는지 본다 (Phase 4 — 주민번호·주소를 다루기 시작).
//
//   화면(src/)        console.* · localStorage · sessionStorage · indexedDB 금지
//                     주민번호를 다루는 작성 화면은 useMutation 금지(캐시가 요청 값을 들고 있다)
//   Rust(src-tauri/)  println! · eprintln! · dbg! 금지, 증명서·주민번호 모듈은 log:: 도 금지

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(fileURLToPath(new URL('.', import.meta.url)), '..')

function walk(p, ext, out = []) {
  if (statSync(p).isDirectory()) {
    for (const n of readdirSync(p)) walk(join(p, n), ext, out)
  } else if (ext.test(p)) out.push(p)
  return out
}

/** 주석 줄은 빼고 본다 (설명에 금지어를 적을 수 있게) */
function codeLines(file) {
  return readFileSync(file, 'utf8')
    .split(/\r?\n/)
    .map((line, i) => ({ line, n: i + 1 }))
    .filter(({ line }) => !/^\s*(\/\/|\*|\/\*)/.test(line))
}

function find(files, re) {
  const hits = []
  for (const f of files) {
    for (const { line, n } of codeLines(f)) if (re.test(line)) hits.push(`${relative(ROOT, f)}:${n}`)
  }
  return hits
}

const ui = walk(join(ROOT, 'src'), /\.(ts|tsx)$/).filter((f) => !f.endsWith('.test.ts'))
const rust = walk(join(ROOT, 'src-tauri', 'src'), /\.rs$/).filter((f) => !f.endsWith('_tests.rs'))

test('화면 코드에 console 출력이 없다', () => {
  assert.deepEqual(find(ui, /\bconsole\s*\./), [])
})

test('화면 코드가 브라우저 저장소를 쓰지 않는다', () => {
  assert.deepEqual(find(ui, /\b(localStorage|sessionStorage|indexedDB)\b/), [])
})

test('증명서 작성 화면은 useMutation 을 쓰지 않는다', () => {
  const issue = ui.filter((f) => /[\\/](IssuePage\.tsx|features[\\/]issued?[\\/])/.test(f))
  assert.ok(issue.length > 0, '작성 화면 파일을 찾지 못했다')
  assert.deepEqual(find(issue, /\buseMutation\b/), [])
})

test('Rust 코드에 println!·eprintln!·dbg! 가 없다', () => {
  assert.deepEqual(find(rust, /\b(println|eprintln|dbg)!\s*\(/), [])
})

test('증명서·주민번호·작성 명령 모듈은 로그를 남기지 않는다', () => {
  const pii = rust.filter((f) => /[\\/](certificate|rrn)\.rs$/.test(f))
  assert.ok(pii.length >= 3, `대상 ${pii.length}개`)
  assert.deepEqual(find(pii, /\blog::/), [])
})
