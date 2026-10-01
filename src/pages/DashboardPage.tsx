import type { AppInfo } from '@/ipc/app'
import s from './Page.module.css'

interface Props {
  info: AppInfo
}

/**
 * Phase 0 대시보드 — 시작 결과와 자료 위치만 보여 준다.
 * 최근 발급·강사 수 같은 요약은 해당 Phase 에서 더한다.
 */
export function DashboardPage({ info }: Props) {
  return (
    <div className={s.page}>
      <div className={s.header}>
        <h1 className={s.title}>대시보드</h1>
      </div>

      {info.sandbox && (
        <div className={s.note} data-tone="warn">
          연습용으로 실행 중입니다. 여기서 입력한 자료는 실제 자료와 따로 저장됩니다.
        </div>
      )}

      {info.notes.map((n, i) => (
        <div key={`${n.kind}-${i}`} className={s.note} data-tone={n.tone}>
          {n.message}
        </div>
      ))}

      <section className={s.card}>
        <h2 className={s.cardTitle}>자료 보관 위치</h2>
        <dl className={s.facts}>
          <dt>자료 파일</dt>
          <dd className="selectable">{info.dbPath}</dd>
          <dt>백업 폴더</dt>
          <dd className="selectable">{info.backupDir}</dd>
          <dt>자료 구조 버전</dt>
          <dd>
            v{info.schemaVersion} (프로그램 지원 v{info.latestSchemaVersion})
          </dd>
        </dl>
        <p className={s.muted}>
          모든 자료는 이 컴퓨터에만 저장됩니다. 프로그램을 켤 때 하루 한 번 자동 백업을 만듭니다.
        </p>
      </section>
    </div>
  )
}
