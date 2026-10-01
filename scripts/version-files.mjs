/**
 * 버전이 적혀 있는 자리들 — 읽기와 쓰기를 한곳에 모아 둔다.
 *
 * `check-version.mjs` 와 `set-version.mjs` 가 같은 목록을 봐야 한다.
 * 한쪽만 알고 있으면 「맞췄는데 시험이 통과하지 않는」 일이 생긴다.
 *
 * 잠금 파일(package-lock.json · Cargo.lock)에도 이 프로그램 자신의 버전이 적혀 있다.
 * 거기까지 맞춰 두지 않으면 `npm ci` 나 `cargo build --locked` 가 도는 CI 에서
 * 잠금 파일이 바뀌었다며 멈춘다.
 */
import { readFileSync, writeFileSync } from 'node:fs'

/** Cargo.lock 안에서 이 프로그램 자신의 항목 (남의 crate 는 건드리지 않는다) */
const CARGO_SELF = /(\[\[package\]\]\r?\nname = "career_cert"\r?\nversion = ")([^"]*)(")/

export const FILES = [
  {
    path: 'package.json',
    read: (t) => JSON.parse(t).version,
    write: (t, v) => {
      const j = JSON.parse(t)
      j.version = v
      return JSON.stringify(j, null, 2) + '\n'
    },
  },
  {
    path: 'package-lock.json',
    read: (t) => {
      const j = JSON.parse(t)
      const root = j.packages?.['']?.version
      // 두 자리가 어긋나 있으면 읽는 단계에서 드러나야 한다
      return root !== undefined && root !== j.version ? `${j.version}/${root}` : j.version
    },
    write: (t, v) => {
      const j = JSON.parse(t)
      j.version = v
      if (j.packages?.['']) j.packages[''].version = v
      return JSON.stringify(j, null, 2) + '\n'
    },
  },
  {
    path: 'src-tauri/tauri.conf.json',
    read: (t) => JSON.parse(t).version,
    write: (t, v) => {
      const j = JSON.parse(t)
      j.version = v
      return JSON.stringify(j, null, 2) + '\n'
    },
  },
  {
    path: 'src-tauri/Cargo.toml',
    read: (t) => (t.match(/^version = "(.*)"$/m) || [])[1],
    write: (t, v) => t.replace(/^version = ".*"$/m, `version = "${v}"`),
  },
  {
    path: 'src-tauri/Cargo.lock',
    read: (t) => (t.match(CARGO_SELF) || [])[2],
    write: (t, v) => {
      if (!CARGO_SELF.test(t)) throw new Error('Cargo.lock 에서 이 프로그램 항목을 찾지 못했습니다.')
      return t.replace(CARGO_SELF, `$1${v}$3`)
    },
  },
]

/** 자리마다 지금 적혀 있는 버전 */
export function readAll() {
  const out = {}
  for (const f of FILES) out[f.path] = f.read(readFileSync(f.path, 'utf8'))
  return out
}

/** 모든 자리를 `next` 로 맞춘다 */
export function writeAll(next) {
  for (const f of FILES) writeFileSync(f.path, f.write(readFileSync(f.path, 'utf8'), next))
}
