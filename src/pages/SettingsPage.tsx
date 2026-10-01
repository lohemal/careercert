import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { FileSpreadsheet, RotateCcw, Save, Wand2 } from 'lucide-react'

import { Badge, Button, Card, ErrorNotice, Field, Input, Notice, Page } from '@/components/ui'
import type { AppInfo } from '@/ipc/app'
import { PortableSection } from '@/features/data/PortableSection'
import { RecoverySection } from '@/features/data/RecoverySection'
import {
  DEFAULT_PURPOSE,
  DEFAULT_TITLE,
  settingsApi,
  suggestIssuer,
  type FieldKey,
  type SchoolSettings,
  type SettingsInput,
} from '@/ipc/settings'
import s from './SettingsPage.module.css'

const KEYS: FieldKey[] = [
  'schoolName',
  'issuerTitle',
  'department',
  'managerName',
  'phone',
  'certificateTitle',
  'defaultPurpose',
]

function toInput(v: SchoolSettings): SettingsInput {
  const { updatedAt: _ignored, ...rest } = v
  return rest
}

/** `2026-10-01T09:05:00` → `2026.10.01. 09:05` */
function stampLabel(iso: string): string {
  const [d, t = ''] = iso.split('T')
  return `${d.replace(/-/g, '.')}. ${t.slice(0, 5)}`
}

interface Props {
  info: AppInfo
}

export function SettingsPage({ info }: Props) {
  const qc = useQueryClient()
  const query = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const saved = query.data?.settings
  const navigate = useNavigate()

  const [form, setForm] = useState<SettingsInput | null>(null)
  // 저장 직후 알릴 말 — 다시 고치기 시작하면 지운다
  const [result, setResult] = useState<{ at: string; issuerFilled: string | null } | null>(null)

  // 처음 불러왔을 때만 칸을 채운다. 고치는 중에 다시 불러와도 입력을 덮지 않는다.
  useEffect(() => {
    if (saved && form === null) setForm(toInput(saved))
  }, [saved, form])

  const save = useMutation({
    mutationFn: settingsApi.save,
    onSuccess: (view) => {
      qc.setQueryData(['settings'], view)
      setForm(toInput(view.settings))
      setResult({
        at: view.settings.updatedAt ?? '',
        issuerFilled: view.issuerFilled ? view.settings.issuerTitle : null,
      })
    },
  })

  const dirty = useMemo(
    () => !!form && !!saved && KEYS.some((k) => form[k] !== saved[k]),
    [form, saved],
  )

  if (query.error) {
    return (
      <Page title="설정">
        <ErrorNotice error={query.error} />
      </Page>
    )
  }
  if (!form || !saved) return <Page title="설정">{null}</Page>

  const set = (k: FieldKey, value: string) => {
    setForm({ ...form, [k]: value })
    setResult(null)
    save.reset()
  }

  // ---- 발급자 표기 도움말 ----
  // 프로그램은 값을 바꾸지 않는다. 사용자가 버튼을 눌렀을 때만 바꾼다.
  const suggestion = suggestIssuer(form.schoolName)
  const issuerBlank = form.issuerTitle.trim() === ''
  // 저장된 발급자 표기가 저장된 학교명에서 만든 값 그대로였고, 학교명만 바뀌었다면 따라 바꿀지 묻는다
  const followSchool =
    !issuerBlank &&
    suggestion !== null &&
    form.schoolName.trim() !== saved.schoolName &&
    form.issuerTitle === saved.issuerTitle &&
    saved.issuerTitle === suggestIssuer(saved.schoolName) &&
    form.issuerTitle !== suggestion

  let issuerHint: ReactNode = '증명서 아래쪽에 찍히는 발급자 이름입니다. 학교명과 따로 저장됩니다.'
  if (issuerBlank && suggestion) {
    issuerHint = (
      <span className={s.hintRow}>
        비워 두면 저장할 때 ‘{suggestion}’(으)로 채웁니다.
        <Button size="sm" variant="ghost" icon={Wand2} onClick={() => set('issuerTitle', suggestion)}>
          지금 채우기
        </Button>
      </span>
    )
  } else if (followSchool) {
    issuerHint = (
      <span className={s.hintRow}>
        학교명을 바꿨습니다. 발급자 표기는 그대로 ‘{form.issuerTitle}’ 입니다.
        <Button size="sm" variant="ghost" icon={Wand2} onClick={() => set('issuerTitle', suggestion!)}>
          ‘{suggestion}’(으)로 바꾸기
        </Button>
      </span>
    )
  }

  const resetButton = (k: 'certificateTitle' | 'defaultPurpose', value: string) =>
    form[k] !== value ? (
      <Button size="sm" variant="ghost" icon={RotateCcw} onClick={() => set(k, value)}>
        기본값으로
      </Button>
    ) : null

  const missing = query.data?.missing ?? []

  return (
    <Page
      title="설정"
      description="학교 기본정보는 증명서를 발급할 때 자동으로 들어갑니다."
      actions={
        <>
          {dirty && <Badge tone="warn">저장하지 않은 변경</Badge>}
          <Button
            variant="primary"
            icon={Save}
            disabled={!dirty || save.isPending}
            onClick={() => save.mutate(form)}
          >
            {save.isPending ? '저장 중…' : '저장'}
          </Button>
        </>
      }
    >
      {save.error ? <ErrorNotice error={save.error} /> : null}
      {result && (
        <Notice tone="success">
          저장했습니다 ({stampLabel(result.at)}).
          {result.issuerFilled && ` 발급자 표기가 비어 있어 ‘${result.issuerFilled}’(으)로 채웠습니다. 다르게 쓰려면 고쳐서 다시 저장하세요.`}
        </Notice>
      )}
      {!dirty && missing.length > 0 && (
        <Notice tone="warn">
          증명서에 들어갈 항목 중 비어 있는 것이 있습니다: {missing.map((m) => m.label).join(', ')}
        </Notice>
      )}

      <Card title="학교 기본정보" description="증명서 아래쪽 발급자·담당자 칸에 들어갑니다.">
        <form
          className={s.grid}
          onSubmit={(e) => {
            e.preventDefault()
            if (dirty) save.mutate(form)
          }}
        >
          <Field label="학교명">
            <Input
              value={form.schoolName}
              placeholder="예: ○○초등학교"
              onChange={(e) => set('schoolName', e.target.value)}
              autoFocus={!saved.schoolName}
            />
          </Field>
          <Field label="발급자 표기" hint={issuerHint}>
            <Input
              value={form.issuerTitle}
              placeholder={suggestion ?? '예: ○○초등학교장'}
              onChange={(e) => set('issuerTitle', e.target.value)}
            />
          </Field>
          <Field label="담당부서">
            <Input
              value={form.department}
              placeholder="예: 방과후학교"
              onChange={(e) => set('department', e.target.value)}
            />
          </Field>
          <Field label="담당자" hint="증명서에는 이름 뒤에 (인) 이 붙습니다.">
            <Input
              value={form.managerName}
              placeholder="예: ○○○"
              onChange={(e) => set('managerName', e.target.value)}
            />
          </Field>
          <Field label="전화번호">
            <Input
              value={form.phone}
              placeholder="예: 000-000-0000"
              onChange={(e) => set('phone', e.target.value)}
            />
          </Field>
          {/* Enter 로도 저장된다 */}
          <button type="submit" hidden />
        </form>
      </Card>

      <Card title="증명서 기본값" description="새 증명서를 작성할 때 처음 채워지는 값입니다. 작성 화면에서 그때그때 바꿀 수 있습니다.">
        <div className={s.grid}>
          <Field
            label="문서 제목"
            className={s.wide}
            hint={
              <span className={s.hintRow}>
                비워 둘 수 없습니다. 기본값: {DEFAULT_TITLE}
                {resetButton('certificateTitle', DEFAULT_TITLE)}
              </span>
            }
          >
            <Input value={form.certificateTitle} onChange={(e) => set('certificateTitle', e.target.value)} />
          </Field>
          <Field
            label="기본 용도"
            hint={
              <span className={s.hintRow}>
                비워 둘 수 없습니다. 기본값: {DEFAULT_PURPOSE}
                {resetButton('defaultPurpose', DEFAULT_PURPOSE)}
              </span>
            }
          >
            <Input value={form.defaultPurpose} onChange={(e) => set('defaultPurpose', e.target.value)} />
          </Field>
        </div>
      </Card>

      <Card title="데이터 관리" description="자료는 이 컴퓨터에만 저장됩니다. 앱 안의 자동 백업은 하루 한 번 자료 폴더의 backups 에 만들어집니다.">
        <RecoverySection />
        <PortableSection />
        <section className={s.dataSection}>
          <h3 className={s.dataTitle}>엑셀 가져오기</h3>
          <p className={s.recoveryNote}>기존 엑셀 강사명단(.xlsx·.xlsm)에서 강사와 경력을 가져옵니다. 미리보기를 거친 뒤에만 반영합니다.</p>
          <div>
            <Button icon={FileSpreadsheet} onClick={() => navigate('/instructors/import')}>
              엑셀 가져오기
            </Button>
          </div>
        </section>
        <section className={s.dataSection}>
          <h3 className={s.dataTitle}>자료 위치</h3>
        <dl className={s.facts}>
          <dt>실행 종류</dt>
          <dd>{info.sandbox ? <Badge tone="warn">연습용 — 실제 자료와 따로 저장</Badge> : '실제 자료'}</dd>
          <dt>자료 파일</dt>
          <dd className={`${s.path} selectable`}>{info.dbPath}</dd>
          <dt>백업 폴더</dt>
          <dd className={`${s.path} selectable`}>{info.backupDir}</dd>
          <dt>자료 구조 버전</dt>
          <dd>
            v{info.schemaVersion} (프로그램 지원 v{info.latestSchemaVersion})
          </dd>
          <dt>입력 자동완성 저장</dt>
          <dd>
            {info.autofillOff === true
              ? '꺼짐 — 입력한 주민번호·주소가 화면 엔진(WebView2)에 저장되지 않습니다'
              : info.autofillOff === false
                ? <Badge tone="error">끄지 못했습니다 — 프로그램을 다시 시작해 주세요</Badge>
                : '확인 중'}
          </dd>
          <dt>마지막 저장</dt>
          <dd>{saved.updatedAt ? stampLabel(saved.updatedAt) : '아직 저장하지 않음'}</dd>
        </dl>
        </section>
      </Card>
    </Page>
  )
}
