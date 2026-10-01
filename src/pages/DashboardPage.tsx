import { useNavigate } from 'react-router-dom'
import { Settings } from 'lucide-react'

import { Button, Card, ErrorNotice, Notice, Page } from '@/components/ui'
import type { AppInfo } from '@/ipc/app'
import type { SettingsView } from '@/ipc/settings'
import s from './DashboardPage.module.css'

interface Props {
  info: AppInfo
  settings: SettingsView | undefined
  settingsError: unknown
}

/**
 * 대시보드 — 지금은 시작 결과와 학교 설정 상태만.
 * 최근 발급·강사 수 같은 요약은 해당 Phase 에서 더한다.
 * 자료 파일 경로는 업무 중 볼 정보가 아니라 설정 → 데이터 관리로 옮겼다.
 */
export function DashboardPage({ info, settings, settingsError }: Props) {
  const navigate = useNavigate()
  const missing = settings?.missing ?? []
  const school = settings?.settings

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

      {school && missing.length === 0 && (
        <Card title="학교 기본정보">
          <dl className={s.facts}>
            <dt>학교명</dt>
            <dd>{school.schoolName}</dd>
            <dt>발급자 표기</dt>
            <dd>{school.issuerTitle}</dd>
            <dt>담당</dt>
            <dd>
              {school.department} · {school.managerName} · {school.phone}
            </dd>
          </dl>
        </Card>
      )}
    </Page>
  )
}
