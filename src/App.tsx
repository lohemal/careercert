import { useQuery } from '@tanstack/react-query'
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom'

import { AppShell } from '@/components/AppShell'
import { appApi } from '@/ipc/app'
import { errorDetail, errorMessage } from '@/ipc/invoke'
import { BulkEndPage } from '@/pages/BulkEndPage'
import { DashboardPage } from '@/pages/DashboardPage'
import { InstructorsPage } from '@/pages/InstructorsPage'
import { IssuePage } from '@/pages/IssuePage'
import { PlaceholderPage } from '@/pages/PlaceholderPage'
import { SettingsPage } from '@/pages/SettingsPage'
import { settingsApi } from '@/ipc/settings'
import s from './App.module.css'

export function App() {
  const info = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })
  // 사이드바·대시보드·설정이 같은 값을 본다 — 설정에서 저장하면 셋이 함께 바뀐다
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })

  if (info.isLoading) {
    return (
      <div className={s.splash}>
        <div className={s.splashMark} />
        <p className={s.splashText}>경력증명서 발급 시스템을 준비하는 중…</p>
      </div>
    )
  }

  if (info.error || !info.data) {
    return (
      <div className={s.splash}>
        <h1 className={s.fatalTitle}>프로그램을 시작하지 못했습니다</h1>
        <p className={s.fatalMsg}>{errorMessage(info.error)}</p>
        {errorDetail(info.error) && (
          <details className={s.fatalDetail}>
            <summary>자세히</summary>
            <pre className="selectable">{errorDetail(info.error)}</pre>
          </details>
        )}
      </div>
    )
  }

  return (
    <HashRouter>
      <Routes>
        <Route
          element={
            <AppShell
              appVersion={info.data.appVersion}
              sandbox={info.data.sandbox}
              schoolName={settings.data?.settings.schoolName}
            />
          }
        >
          <Route
            path="/dashboard"
            element={<DashboardPage info={info.data} settings={settings.data} settingsError={settings.error} />}
          />
          <Route path="/instructors" element={<InstructorsPage />} />
          <Route path="/instructors/bulk-end" element={<BulkEndPage />} />
          <Route path="/issue" element={<IssuePage />} />
          <Route
            path="/history"
            element={
              <PlaceholderPage
                title="발급이력"
                phase="Phase 7"
                description="발급된 증명서 검색·열람, 발급 당시 내용 그대로 재출력·PDF 저장."
              />
            }
          />
          <Route path="/settings" element={<SettingsPage info={info.data} />} />
        </Route>
        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </HashRouter>
  )
}
