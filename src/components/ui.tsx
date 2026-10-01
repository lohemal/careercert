import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from 'react'
import { AlertTriangle, CheckCircle2, Info, XCircle, type LucideIcon } from 'lucide-react'

import { errorDetail, errorMessage } from '@/ipc/invoke'
import s from './ui.module.css'

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(' ')
}

/* ---------- Page ---------- */

export function Page({
  title,
  description,
  badge,
  actions,
  children,
}: {
  title: string
  description?: string
  /** 제목 옆 작은 표시 (예: 구현 예정 Phase) */
  badge?: ReactNode
  actions?: ReactNode
  children: ReactNode
}) {
  return (
    <div className={s.page}>
      <header className={s.topbar}>
        <h1 className={s.topbarTitle}>{title}</h1>
        {badge}
        {description && <span className={s.topbarDesc}>{description}</span>}
        <span className={s.topbarSpacer} />
        {actions && <div className={s.topbarActions}>{actions}</div>}
      </header>
      <div className={s.body}>{children}</div>
    </div>
  )
}

/* ---------- Card ---------- */

export function Card({
  title,
  description,
  actions,
  children,
  className,
}: {
  title?: string
  description?: string
  actions?: ReactNode
  children: ReactNode
  className?: string
}) {
  return (
    <section className={cx(s.card, className)}>
      {(title || actions) && (
        <header className={s.cardHead}>
          <div>
            {title && <h2 className={s.cardTitle}>{title}</h2>}
            {description && <p className={s.cardDesc}>{description}</p>}
          </div>
          {actions}
        </header>
      )}
      <div className={s.cardBody}>{children}</div>
    </section>
  )
}

/* ---------- Button ---------- */

type Variant = 'primary' | 'secondary' | 'outline' | 'ghost' | 'danger'

export function Button({
  variant = 'secondary',
  size,
  icon: IconCmp,
  children,
  className,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant
  size?: 'sm'
  icon?: LucideIcon
}) {
  return (
    <button
      type="button"
      className={cx(s.btn, s[variant], size === 'sm' && s.btnSm, className)}
      {...rest}
    >
      {IconCmp && <IconCmp size={size === 'sm' ? 14 : 16} strokeWidth={2} />}
      {children}
    </button>
  )
}

/* ---------- Form ---------- */

export function Field({
  label,
  hint,
  className,
  children,
}: {
  label: string
  hint?: ReactNode
  /** 격자 안에서 칸을 더 차지해야 할 때 (예: 주소) */
  className?: string
  children: ReactNode
}) {
  return (
    <label className={cx(s.field, className)}>
      <span className={s.label}>{label}</span>
      {children}
      {hint && <span className={s.hint}>{hint}</span>}
    </label>
  )
}

export function Input({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cx(s.input, className)} {...rest} />
}

/* ---------- Badge ---------- */

export type Tone = 'success' | 'info' | 'warn' | 'error' | 'neutral'

const BADGE: Record<Tone, string> = {
  success: s.badgeSuccess,
  info: s.badgeInfo,
  warn: s.badgeWarn,
  error: s.badgeError,
  neutral: s.badgeNeutral,
}

export function Badge({ tone = 'neutral', children }: { tone?: Tone; children: ReactNode }) {
  return <span className={cx(s.badge, BADGE[tone])}>{children}</span>
}

/* ---------- Notice ---------- */

const NOTICE: Record<Exclude<Tone, 'neutral'>, { cls: string; icon: LucideIcon }> = {
  info: { cls: s.noticeInfo, icon: Info },
  warn: { cls: s.noticeWarn, icon: AlertTriangle },
  error: { cls: s.noticeError, icon: XCircle },
  success: { cls: s.noticeSuccess, icon: CheckCircle2 },
}

export function Notice({
  tone = 'info',
  children,
  detail,
}: {
  tone?: Exclude<Tone, 'neutral'>
  children: ReactNode
  detail?: string | null
}) {
  const { cls, icon: IconCmp } = NOTICE[tone]
  return (
    <div className={cx(s.notice, cls)} role={tone === 'error' ? 'alert' : 'status'}>
      <IconCmp size={16} className={s.noticeIcon} />
      <div>
        <div>{children}</div>
        {detail && (
          <details className={s.noticeDetail}>
            <summary>자세히</summary>
            <pre className="selectable">{detail}</pre>
          </details>
        )}
      </div>
    </div>
  )
}

/** 명령 오류를 그대로 보여주는 안내 */
export function ErrorNotice({ error }: { error: unknown }) {
  if (!error) return null
  return (
    <Notice tone="error" detail={errorDetail(error)}>
      {errorMessage(error)}
    </Notice>
  )
}
