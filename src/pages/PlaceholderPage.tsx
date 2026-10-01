import s from './Page.module.css'

interface Props {
  title: string
  /** 어느 Phase 에서 만드는가 */
  phase: string
  description: string
}

/** Phase 0 — 메뉴 자리만 잡아 둔 빈 화면 */
export function PlaceholderPage({ title, phase, description }: Props) {
  return (
    <div className={s.page}>
      <div className={s.header}>
        <h1 className={s.title}>{title}</h1>
        <span className={s.phase}>{phase} 에서 구현</span>
      </div>
      <div className={s.card}>
        <p className={s.muted}>{description}</p>
      </div>
    </div>
  )
}
