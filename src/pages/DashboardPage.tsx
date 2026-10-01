import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { History, Settings } from 'lucide-react'

import { Button, Card, ErrorNotice, Notice, Page } from '@/components/ui'
import { whoLabel } from '@/features/instructors/label'
import type { AppInfo } from '@/ipc/app'
import { dashboardApi } from '@/ipc/dashboard'
import { historyApi, type HistoryRow } from '@/ipc/issuance'
import type { SettingsView } from '@/ipc/settings'
import s from './DashboardPage.module.css'

interface Props {
  info: AppInfo
  settings: SettingsView | undefined
  settingsError: unknown
}

/**
 * 대시보드 — 시작 결과 · 학교 설정 상태 · 강사 요약 · 확인 필요.
 * "확인 필요" 는 지금 자료로 **객관적으로 가릴 수 있는 것만** 보이고, 아무것도 바꾸지 않는다(Rust `service::dashboard`).
 * 발급 현황은 최근 발급 5건 · 최근 취소 5건만(Phase 7 결정 — 기간별 통계·차트는 없음).
 */
export function DashboardPage({ info, settings, settingsError }: Props) {
  const navigate = useNavigate()
  const overview = useQuery({ queryKey: ['dashboard'], queryFn: dashboardApi.overview })
  const missing = settings?.missing ?? []
  const school = settings?.settings
  const o = overview.data
  const recent = useQuery({ queryKey: ['certificate-recent'], queryFn: historyApi.recent })
  const r = recent.data
  const openCert = (id: number) => navigate(`/history?id=${id}`)

  return (
    <Page title="대시보드">
      {info.sandbox && (
        <Notice tone="warn">연습용으로 실행 중입니다. 여기서 입력한 자료는 실제 자료와 따로 저장됩니다.</Notice>
      )}
      {info.notes.map((n, i) => (
        <Notice key={`${n.kind}-${i}`} tone={n.tone}>
          {n.message}
        </Notice>
      ))}
      <ErrorNotice error={settingsError} />
      <ErrorNotice error={overview.error} />

      {settings && missing.length > 0 && (
        <Notice tone="warn">
          <div className={s.setup}>
            <span>
              학교 기본정보가 아직 다 채워지지 않았습니다. 증명서를 발급하기 전에 설정해 주세요.
              <br />
              <span className={s.missing}>비어 있는 항목: {missing.map((m) => m.label).join(', ')}</span>
            </span>
            <Button size="sm" variant="primary" icon={Settings} onClick={() => navigate('/settings')}>
              설정으로 이동
            </Button>
          </div>
        </Notice>
      )}

      <div className={s.tiles}>
        <button type="button" className={s.tile} onClick={() => navigate('/instructors')}>
          <span className={s.tileLabel}>등록 강사</span>
          <span className={s.tileValue}>{o ? `${o.instructors}명` : '—'}</span>
          <span className={s.tileNote}>보관한 강사 제외</span>
        </button>
        <button type="button" className={s.tile} onClick={() => navigate('/instructors')}>
          <span className={s.tileLabel}>재직중 경력</span>
          <span className={s.tileValue}>{o ? `${o.activeCareers}건` : '—'}</span>
          <span className={s.tileNote}>종료 처리하지 않은 경력</span>
        </button>
        {school && missing.length === 0 && (
          <div className={s.tileWide}>
            <span className={s.tileLabel}>학교 기본정보</span>
            <span className={s.schoolName}>{school.schoolName}</span>
            <span className={s.tileNote}>
              {school.issuerTitle} · {school.department} · {school.managerName} · {school.phone}
            </span>
          </div>
        )}
      </div>

      <ErrorNotice error={recent.error} />
      {r && (
        <Card
          title="발급 현황"
          description={`발급 ${r.issuedCount}건 · 취소 ${r.voidedCount}건`}
          actions={
            <Button size="sm" icon={History} onClick={() => navigate('/history')}>
              발급이력
            </Button>
          }
        >
          <div className={s.recent}>
            <RecentList title="최근 발급" rows={r.issued} empty="아직 발급한 증명서가 없습니다." onOpen={openCert} />
            <RecentList title="최근 취소" rows={r.voided} empty="취소한 발급 건이 없습니다." onOpen={openCert} voided />
          </div>
        </Card>
      )}

      {o && o.checks.length > 0 && (
        <Card title="확인 필요" description="자료에서 확인해 볼 만한 것을 알려 드립니다. 프로그램이 저절로 고치지 않습니다.">
          <div className={s.checks}>
            {o.checks.map((c) => (
              <section key={c.kind} className={s.check}>
                <h3 className={s.checkTitle}>
                  {c.title} <span className={s.count}>{c.items.length}</span>
                </h3>
                <p className={s.checkDesc}>{c.description}</p>
                <ul className={s.items}>
                  {c.items.map((it, n) => (
                    <li key={`${it.instructorId}-${n}`}>
                      <button type="button" className={s.link} onClick={() => navigate(`/instructors?id=${it.instructorId}`)}>
                        {whoLabel({ name: it.instructorName, distinguisher: it.distinguisher })}
                      </button>
                      <span className={s.detail}>{it.detail}</span>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
        </Card>
      )}
    </Page>
  )
}

function RecentList({
  title,
  rows,
  empty,
  voided = false,
  onOpen,
}: {
  title: string
  rows: HistoryRow[]
  empty: string
  voided?: boolean
  onOpen: (id: number) => void
}) {
  return (
    <section>
      <h3 className={s.checkTitle}>{title}</h3>
      {rows.length === 0 ? (
        <p className={s.checkDesc}>{empty}</p>
      ) : (
        <ul className={s.items}>
          {rows.map((row) => (
            <li key={row.id}>
              <button type="button" className={s.link} onClick={() => onOpen(row.id)}>
                {row.issueNo}
              </button>
              <span className={s.detail}>
                {row.holderName} · {voided && row.voidedAt ? `${row.voidedAt.slice(0, 10).replace(/-/g, '.')} 취소` : row.issuedOnLabel}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
