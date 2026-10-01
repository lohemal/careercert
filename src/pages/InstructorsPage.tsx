import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { Archive, ArchiveRestore, CalendarCheck, CalendarX2, Pencil, Plus, Search, UserPlus } from 'lucide-react'

import { Badge, Button, ErrorNotice, Input, Notice, Page } from '@/components/ui'
import { useConfirm } from '@/components/useConfirm'
import { CareerDialog } from '@/features/instructors/CareerDialog'
import { EndCareerDialog } from '@/features/instructors/EndCareerDialog'
import { InstructorDialog } from '@/features/instructors/InstructorDialog'
import { invalidateCareerData } from '@/features/instructors/invalidate'
import { whoLabel } from '@/features/instructors/label'
import { careerApi, type Career } from '@/ipc/career'
import { instructorApi, type InstructorRow, type Scope } from '@/ipc/instructor'
import s from './InstructorsPage.module.css'

const SCOPES: { value: Scope; label: string }[] = [
  { value: 'ACTIVE', label: '재직중' },
  { value: 'ALL', label: '전체' },
  { value: 'ARCHIVED', label: '보관' },
]

export function InstructorsPage() {
  const navigate = useNavigate()
  const [params, setParams] = useSearchParams()
  const selectedId = Number(params.get('id')) || null
  const [query, setQuery] = useState('')
  const [scope, setScope] = useState<Scope>('ALL')
  const [adding, setAdding] = useState(false)

  const list = useQuery({
    queryKey: ['instructors', query.trim(), scope],
    queryFn: () => instructorApi.search(query.trim(), scope),
  })

  const select = (id: number) => setParams({ id: String(id) }, { replace: true })

  return (
    <Page
      title="강사관리"
      actions={
        <Button icon={CalendarCheck} onClick={() => navigate('/instructors/bulk-end')}>
          재직중 경력 일괄 종료
        </Button>
      }
    >
      <div className={s.panes}>
        {/* ---------- 왼쪽: 찾기 · 목록 ---------- */}
        <aside className={s.left}>
          <div className={s.leftTools}>
            <div className={s.searchBox}>
              <Search size={16} className={s.searchIcon} />
              <Input
                className={s.searchInput}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="검색"
                title="이름이나 구분 메모로 찾습니다"
              />
            </div>
            <Button variant="primary" icon={UserPlus} onClick={() => setAdding(true)}>
              강사 추가
            </Button>
          </div>
          <div className={s.scopes} role="tablist">
            {SCOPES.map((o) => (
              <button
                key={o.value}
                type="button"
                role="tab"
                aria-selected={scope === o.value}
                className={scope === o.value ? s.scopeOn : s.scope}
                onClick={() => setScope(o.value)}
              >
                {o.label}
              </button>
            ))}
          </div>
          <ErrorNotice error={list.error} />
          <div className={s.list}>
            {list.data?.length === 0 && (
              <p className={s.empty}>
                {query.trim() ? '찾는 강사가 없습니다.' : scope === 'ARCHIVED' ? '보관한 강사가 없습니다.' : '등록된 강사가 없습니다.'}
              </p>
            )}
            {list.data?.map((row) => (
              <ListRow key={row.id} row={row} active={row.id === selectedId} onClick={() => select(row.id)} />
            ))}
          </div>
          {list.data && list.data.length > 0 && <div className={s.listFoot}>{list.data.length}명</div>}
        </aside>

        {/* ---------- 오른쪽: 상세 ---------- */}
        <section className={s.right}>
          {selectedId ? (
            <InstructorDetail key={selectedId} id={selectedId} />
          ) : (
            <p className={s.placeholder}>왼쪽 목록에서 강사를 고르세요.</p>
          )}
        </section>
      </div>

      {adding && (
        <InstructorDialog
          onClose={() => setAdding(false)}
          onSaved={(saved) => {
            setAdding(false)
            select(saved.id)
          }}
        />
      )}
    </Page>
  )
}

function ListRow({ row, active, onClick }: { row: InstructorRow; active: boolean; onClick: () => void }) {
  return (
    <button type="button" className={active ? s.rowOn : s.row} onClick={onClick}>
      <span className={s.rowTop}>
        <span className={s.rowName}>{row.name}</span>
        {row.distinguisher && <span className={row.sameName ? s.distStrong : s.dist}>{row.distinguisher}</span>}
        {row.sameName && !row.distinguisher && <Badge tone="warn">동명이인 · 구분 메모 없음</Badge>}
        {row.archived && <Badge tone="neutral">보관</Badge>}
      </span>
      <span className={s.rowMeta}>
        경력 {row.careers}건{row.activeCareers > 0 ? ` · 재직중 ${row.activeCareers}건` : ''}
      </span>
    </button>
  )
}

function InstructorDetail({ id }: { id: number }) {
  const qc = useQueryClient()
  const [confirm, confirmDialog] = useConfirm()
  const [showArchived, setShowArchived] = useState(false)
  const [editing, setEditing] = useState(false)
  const [careerForm, setCareerForm] = useState<{ career?: Career } | null>(null)
  const [ending, setEnding] = useState<Career | null>(null)

  const who = useQuery({ queryKey: ['instructor', id], queryFn: () => instructorApi.get(id) })
  const careers = useQuery({
    queryKey: ['careers', id, showArchived],
    queryFn: () => careerApi.list(id, showArchived),
  })
  const hints = useQuery({ queryKey: ['career-hints'], queryFn: () => careerApi.hints(''), staleTime: Infinity })

  const act = useMutation({
    mutationFn: (f: () => Promise<unknown>) => f(),
    onSuccess: () => invalidateCareerData(qc),
  })

  if (who.error) return <ErrorNotice error={who.error} />
  const i = who.data
  if (!i) return null
  const locked = i.archived

  const archiveInstructor = async () => {
    const ok = await confirm({
      title: '강사 보관',
      message: (
        <>
          ‘{whoLabel(i)}’ 을(를) 보관합니다. 기본 목록에서 빠지고 [보관] 필터에서 볼 수 있습니다.
          <br />
          지우는 것이 아니며 경력은 그대로 남습니다. [보관 해제] 로 다시 쓸 수 있습니다.
        </>
      ),
      confirmLabel: '보관',
    })
    if (ok) act.mutate(() => instructorApi.archive(i.id))
  }

  const archiveCareer = async (c: Career) => {
    const ok = await confirm({
      title: '경력 보관',
      message: (
        <>
          ‘{c.programName} · {c.period}’ 경력을 보관합니다. 목록에서 빠지고 [보관한 경력도 보기] 로 볼 수 있습니다.
          <br />
          잘못 넣은 경력을 치울 때 씁니다. 지우는 것이 아닙니다.
        </>
      ),
      confirmLabel: '보관',
    })
    if (ok) act.mutate(() => careerApi.archive(c.id))
  }

  return (
    <div className={s.detail}>
      <header className={s.detailHead}>
        <div className={s.detailTitle}>
          <h2>{i.name}</h2>
          {i.distinguisher && <span className={s.distStrong}>{i.distinguisher}</span>}
          {i.archived && <Badge tone="neutral">보관됨</Badge>}
        </div>
        <div className={s.detailActions}>
          <Button icon={Pencil} onClick={() => setEditing(true)} disabled={locked}>
            정보 수정
          </Button>
          {i.archived ? (
            <Button icon={ArchiveRestore} onClick={() => act.mutate(() => instructorApi.unarchive(i.id))}>
              보관 해제
            </Button>
          ) : (
            <Button icon={Archive} onClick={archiveInstructor}>
              보관
            </Button>
          )}
        </div>
      </header>

      {i.archived && (
        <Notice tone="warn">
          보관된 강사입니다. 정보와 경력을 고치거나 경력을 더하려면 먼저 [보관 해제] 를 누르세요.
        </Notice>
      )}
      <ErrorNotice error={act.error} />

      <dl className={s.info}>
        <dt>전화번호</dt>
        <dd>{i.phone || <span className={s.muted}>—</span>}</dd>
        <dt>메모</dt>
        <dd className={s.memo}>{i.memo || <span className={s.muted}>—</span>}</dd>
      </dl>

      <div className={s.careerHead}>
        <h3>경력 {careers.data ? `${careers.data.filter((c) => !c.archived).length}건` : ''}</h3>
        <label className={s.check}>
          <input type="checkbox" checked={showArchived} onChange={(e) => setShowArchived(e.target.checked)} />
          보관한 경력도 보기
        </label>
        <span className={s.spacer} />
        <Button variant="primary" icon={Plus} onClick={() => setCareerForm({})} disabled={locked || !hints.data}>
          경력 추가
        </Button>
      </div>

      <ErrorNotice error={careers.error} />
      <div className={s.tableWrap}>
        <table className={s.table}>
          <thead>
            <tr>
              <th className={s.colPeriod}>기간</th>
              <th>프로그램명</th>
              <th>직위</th>
              <th className={s.colDuty}>지도사항</th>
              <th>상태 · 종료 사유</th>
              <th className={s.colActions} aria-label="작업" />
            </tr>
          </thead>
          <tbody>
            {careers.data?.length === 0 && (
              <tr>
                <td colSpan={6} className={s.emptyCell}>
                  등록된 경력이 없습니다. [경력 추가] 로 넣어 주세요.
                </td>
              </tr>
            )}
            {careers.data?.map((c) => (
              <tr key={c.id} className={c.archived ? s.rowArchived : undefined}>
                <td className={s.period} title={c.period}>
                  {/* 좁으면 "~" 뒤에서만 줄이 바뀐다 */}
                  <span className={s.nowrap}>{c.periodFrom} ~</span> <span className={s.nowrap}>{c.periodTo}</span>
                </td>
                <td>{c.programName}</td>
                <td className={s.nowrap}>{c.position}</td>
                <td>{c.duty}</td>
                <td>
                  <span className={s.badges}>
                    <Badge tone={c.status === 'ACTIVE' ? 'success' : 'neutral'}>{c.statusLabel}</Badge>
                    {c.futureEnd && <Badge tone="warn">종료 예정</Badge>}
                    {c.archived && <Badge tone="neutral">보관</Badge>}
                  </span>
                  {c.endReasonLabel && <span className={s.reason}>{c.endReasonLabel}</span>}
                </td>
                <td className={s.actions}>
                  <div className={s.actionsInner}>
                  {c.archived ? (
                    <Button size="sm" variant="ghost" icon={ArchiveRestore} onClick={() => act.mutate(() => careerApi.unarchive(c.id))}>
                      보관 해제
                    </Button>
                  ) : (
                    <>
                      {c.status === 'ACTIVE' && (
                        <Button size="sm" variant="outline" icon={CalendarX2} onClick={() => setEnding(c)} disabled={locked}>
                          종료 처리
                        </Button>
                      )}
                      <Button
                        size="sm"
                        variant="ghost"
                        icon={Pencil}
                        onClick={() => setCareerForm({ career: c })}
                        disabled={locked}
                        title="경력 수정"
                        aria-label="경력 수정"
                      />
                      <Button
                        size="sm"
                        variant="ghost"
                        icon={Archive}
                        onClick={() => archiveCareer(c)}
                        title="경력 보관"
                        aria-label="경력 보관"
                      />
                    </>
                  )}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {editing && <InstructorDialog instructor={i} onClose={() => setEditing(false)} onSaved={() => setEditing(false)} />}
      {careerForm && hints.data && (
        <CareerDialog
          instructorId={i.id}
          career={careerForm.career}
          defaultPosition={hints.data.defaultPosition}
          onClose={() => setCareerForm(null)}
        />
      )}
      {ending && <EndCareerDialog career={ending} instructorLabel={whoLabel(i)} onClose={() => setEnding(null)} />}
      {confirmDialog}
    </div>
  )
}

