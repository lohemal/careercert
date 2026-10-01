import { NavLink, Outlet } from 'react-router-dom'
import {
  FileBadge,
  FilePlus2,
  History,
  LayoutDashboard,
  Settings,
  Users,
  type LucideIcon,
} from 'lucide-react'

import s from './AppShell.module.css'

interface MenuItem {
  to: string
  label: string
  icon: LucideIcon
}

interface MenuGroup {
  label: string
  items: MenuItem[]
}

/** 업무 순서대로 위에서 아래 (설계안 §5) */
const MENU: MenuGroup[] = [
  {
    label: '업무',
    items: [
      { to: '/dashboard', label: '대시보드', icon: LayoutDashboard },
      { to: '/instructors', label: '강사관리', icon: Users },
      { to: '/issue', label: '증명서 발급', icon: FilePlus2 },
      { to: '/history', label: '발급이력', icon: History },
    ],
  },
  {
    label: '관리',
    items: [{ to: '/settings', label: '설정', icon: Settings }],
  },
]

interface Props {
  appVersion: string
  sandbox: boolean
}

export function AppShell({ appVersion, sandbox }: Props) {
  return (
    <div className={s.shell}>
      <aside className={s.sidebar}>
        <div className={s.brand}>
          <div className={s.brandMark} aria-hidden="true">
            <FileBadge size={20} strokeWidth={2.4} />
          </div>
          <div className={s.brandText}>
            <span className={s.brandTitle}>경력증명서 발급</span>
            {/* 학교 이름은 Phase 1 설정에서 온다 — 코드에 적지 않는다 */}
            <span className={s.brandSub}>학교 미설정</span>
          </div>
        </div>

        <nav className={s.nav}>
          {MENU.map((g) => (
            <div key={g.label} className={s.group}>
              <div className={s.groupLabel}>{g.label}</div>
              {g.items.map((m) => (
                <NavLink
                  key={m.to}
                  to={m.to}
                  className={({ isActive }) => (isActive ? `${s.navItem} ${s.navItemActive}` : s.navItem)}
                >
                  <m.icon size={18} strokeWidth={2} />
                  <span className={s.navLabel}>{m.label}</span>
                </NavLink>
              ))}
            </div>
          ))}
        </nav>

        <div className={s.sidebarFoot}>
          {sandbox ? <span className={s.sandboxChip}>연습용</span> : <span />}
          <span className={s.version}>v{appVersion}</span>
        </div>
      </aside>

      <main className={s.main}>
        <Outlet />
      </main>
    </div>
  )
}
