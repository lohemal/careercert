/**
 * 릴리스에 올릴 파일들을 모은다.
 *
 *   node scripts/make-release.mjs <버전> [저장소]
 *   node scripts/make-release.mjs 0.1.0 lohemal/careercert
 *
 * 앱 이름이 한글이라 만들어진 설치 파일 이름도 한글이다.
 * GitHub 릴리스는 첨부 파일 이름에서 한글을 떼어 내므로 **영문 이름으로 복사**하고,
 * `latest.json` 의 url 도 그 이름을 가리키게 한다. 이것이 어긋나면 업데이터가
 * 파일을 못 찾아 **조용히 깨진다.**
 *
 * PowerShell 대신 Node 로 만든다. Windows PowerShell 5.1 은 한글을 깨뜨리고
 * `utf8NoBOM` 도 없다. `latest.json` 에 BOM 이 붙으면 업데이터가 읽지 못한다.
 */
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, copyFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const version = process.argv[2]
const repo = process.argv[3] ?? 'lohemal/careercert'
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('사용법: node scripts/make-release.mjs 0.1.0 [소유자/저장소]')
  process.exit(1)
}

const bundleDir = 'src-tauri/target/release/bundle/nsis'
if (!existsSync(bundleDir)) {
  console.error(`설치 파일 폴더가 없습니다: ${bundleDir}\n먼저 npm run app:build 를 하세요.`)
  process.exit(1)
}

// **이 버전의** 설치 파일만 고른다.
// 전에 만든 판이 폴더에 남아 있으면 첫 번째를 집어 엉뚱한 버전을 릴리스하게 된다.
const setup = readdirSync(bundleDir).find((f) => f.endsWith(`_${version}_x64-setup.exe`))
if (!setup) {
  const others = readdirSync(bundleDir).filter((f) => f.endsWith('_x64-setup.exe'))
  console.error(
    `v${version} 설치 파일을 찾지 못했습니다: ${bundleDir}` +
      (others.length ? `\n폴더에 있는 것: ${others.join(', ')}` : ''),
  )
  process.exit(1)
}
const sigFile = `${setup}.sig`
if (!existsSync(join(bundleDir, sigFile))) {
  console.error(
    `서명 파일이 없습니다: ${sigFile}\n` +
      'TAURI_SIGNING_PRIVATE_KEY 에 키 파일의 **내용**이 들어갔는지 확인하세요.\n' +
      '(PATH 로 주면 빌드는 성공하는데 서명만 조용히 빠집니다.)',
  )
  process.exit(1)
}
const signature = readFileSync(join(bundleDir, sigFile), 'utf8').trim()
if (signature === '') {
  console.error('서명이 비어 있습니다.')
  process.exit(1)
}

const out = 'release'
rmSync(out, { recursive: true, force: true })
mkdirSync(out, { recursive: true })

const name = `careercert_${version}_x64-setup.exe`
copyFileSync(join(bundleDir, setup), join(out, name))
copyFileSync(join(bundleDir, sigFile), join(out, `${name}.sig`))

const latest = {
  version,
  notes: '자세한 변경 내용은 릴리스 페이지를 확인해 주세요.',
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
  platforms: {
    'windows-x86_64': {
      signature,
      url: `https://github.com/${repo}/releases/download/v${version}/${name}`,
    },
  },
}
// BOM 없이 쓴다 — BOM 이 있으면 업데이터가 읽지 못한다
writeFileSync(join(out, 'latest.json'), JSON.stringify(latest, null, 2) + '\n', 'utf8')

const bytes = readFileSync(join(out, name))
const sha = createHash('sha256').update(bytes).digest('hex')
writeFileSync(join(out, 'SHA256SUMS.txt'), `${sha}  ${name}\n`, 'utf8')

console.log('만든 파일:')
for (const f of readdirSync(out)) {
  const n = readFileSync(join(out, f)).length
  console.log(`  ${f.padEnd(52)} ${n.toLocaleString()} bytes`)
}
console.log('')
console.log(`원본 설치 파일: ${setup}`)
console.log(`SHA-256      : ${sha}`)
