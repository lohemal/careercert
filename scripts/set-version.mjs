/**
 * 버전이 적힌 자리를 한 번에 맞춘다.
 *
 *   npm run version:set 0.2.0
 *
 * package.json · package-lock.json · tauri.conf.json · Cargo.toml · Cargo.lock
 * 이 어긋나면 업데이터가 새 버전을 알아보지 못한다.
 * 손으로 고치지 않고 이 스크립트를 쓴다.
 */
import { readAll, writeAll } from './version-files.mjs'

const next = process.argv[2]
if (!next || !/^\d+\.\d+\.\d+$/.test(next)) {
  console.error('사용법: npm run version:set 0.2.0')
  process.exit(1)
}

const before = readAll()
writeAll(next)
const after = readAll()

console.log(`v${before['package.json']} -> v${next}`)
for (const where of Object.keys(after)) console.log(`  ${where.padEnd(26)} ${after[where]}`)
console.log('')
console.log('다음 단계:')
console.log('  npm run check:version')
console.log(`  git add . && git commit -m "v${next}"`)
