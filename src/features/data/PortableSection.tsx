import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { ArchiveRestore, FileDown, FolderOpen, RotateCw } from 'lucide-react'

import { Badge, Button, ErrorNotice, Field, Input, Modal, Notice } from '@/components/ui'
import { backupApi, type BackupHeader, type Counts, type RestorePreview } from '@/ipc/data'
import { historyApi } from '@/ipc/issuance'
import s from './data.module.css'

/**
 * 이동용 백업(파일 전체를 복구 비밀번호로 암호화) 내보내기 · 복원.
 * 자동 백업(앱 자료 폴더의 backups)과 다르다 — 그것은 이 PC 안에서 되돌리기용이다.
 */
export function PortableSection() {
  const recovery = useQuery({ queryKey: ['recovery'], queryFn: historyApi.recovery, staleTime: 0 })
  const ready = !!recovery.data?.ready
  const [exporting, setExporting] = useState(false)
  const [restoring, setRestoring] = useState(false)
  const [done, setDone] = useState<string | null>(null)

  return (
    <section className={s.section}>
      <h3 className={s.title}>이동용 백업</h3>
      <p className={s.help}>
        다른 PC 로 옮기거나 USB 등 밖에 보관할 백업입니다. 파일 <b>전체</b>를 복구 비밀번호로 암호화하므로 파일을 열어도 강사 이름·연락처·경력이
        보이지 않습니다. 앱 안의 자동 백업은 이 PC 안에서 되돌리기용이라 따로 하루 한 번 만들어집니다.
      </p>
      {!ready && <Notice tone="info">이동용 백업을 만들려면 먼저 복구 비밀번호를 설정해야 합니다.</Notice>}
      {done && <Notice tone="success">{done}</Notice>}
      <div className={s.actions}>
        <Button icon={FileDown} onClick={() => setExporting(true)} disabled={!ready}>
          백업 내보내기
        </Button>
        <Button icon={ArchiveRestore} onClick={() => setRestoring(true)}>
          백업 복원
        </Button>
      </div>
      {exporting && (
        <ExportDialog
          onClose={() => setExporting(false)}
          onDone={(name) => {
            setExporting(false)
            setDone(`이동용 백업을 저장했습니다: ${name}`)
          }}
        />
      )}
      {restoring && <RestoreDialog onClose={() => setRestoring(false)} />}
    </section>
  )
}

function ExportDialog({ onClose, onDone }: { onClose: () => void; onDone: (fileName: string) => void }) {
  const [pw, setPw] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<unknown>(null)

  const run = async () => {
    setBusy(true)
    setError(null)
    try {
      const r = await backupApi.exportPortable(pw)
      setPw('')
      if (r.saved && r.fileName) onDone(r.fileName)
    } catch (e) {
      setError(e)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      title="이동용 백업 내보내기"
      onClose={onClose}
      busy={busy}
      width={500}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            닫기
          </Button>
          <Button variant="primary" icon={FileDown} onClick={run} disabled={busy || pw.length === 0}>
            {busy ? '만드는 중…' : '저장 위치 고르고 만들기'}
          </Button>
        </>
      }
    >
      <p className={s.help}>
        복구 비밀번호가 맞는지 먼저 확인한 뒤 저장 위치를 묻습니다. 만든 파일은 다시 열어 확인까지 합니다. 파일 이름에는 개인정보를 넣지 않습니다
        (예: 방과후강사경력관리_백업_20261001.careercert-backup).
      </p>
      <Field label="복구 비밀번호">
        <Input type="password" autoComplete="off" value={pw} onChange={(e) => setPw(e.target.value)} />
      </Field>
      <ErrorNotice error={error} />
    </Modal>
  )
}

const COUNT_ROWS: { key: keyof Counts; label: string }[] = [
  { key: 'instructors', label: '강사' },
  { key: 'careers', label: '경력' },
  { key: 'certificatesIssued', label: '발급' },
  { key: 'certificatesVoided', label: '취소' },
  { key: 'imports', label: '엑셀 가져오기 기록' },
]

/**
 * 복원 — ① 파일 고르기(머리 검사) → ② 비밀번호로 열어 모든 검사·발급본 검증(지금 자료 그대로) → ③ 미리보기·최종 확인
 * → 지금 자료를 before_restore 로 백업하고 다시 시작하면서 교체.
 */
function RestoreDialog({ onClose }: { onClose: () => void }) {
  const [header, setHeader] = useState<BackupHeader | null>(null)
  const [pw, setPw] = useState('')
  const [preview, setPreview] = useState<RestorePreview | null>(null)
  const [agree, setAgree] = useState(false)
  const [busy, setBusy] = useState<string | null>(null)
  const [restarting, setRestarting] = useState(false)
  const [error, setError] = useState<unknown>(null)

  const close = () => {
    void backupApi.cancel()
    onClose()
  }

  const pick = async () => {
    setBusy('파일을 읽는 중…')
    setError(null)
    setPreview(null)
    setAgree(false)
    try {
      const h = await backupApi.pick()
      if (h) setHeader(h)
    } catch (e) {
      setHeader(null)
      setError(e)
    } finally {
      setBusy(null)
    }
  }

  const unlock = async () => {
    setBusy('확인 중… 백업을 풀고 모든 발급 기록을 열어 봅니다')
    setError(null)
    try {
      setPreview(await backupApi.unlock(pw))
    } catch (e) {
      setError(e)
    } finally {
      setPw('')
      setBusy(null)
    }
  }

  const confirm = async () => {
    setBusy('지금 자료를 백업하고 복원을 준비하는 중…')
    setError(null)
    try {
      await backupApi.confirm()
      setRestarting(true)
    } catch (e) {
      setError(e)
    } finally {
      setBusy(null)
    }
  }

  return (
    <Modal
      title="백업 복원"
      onClose={close}
      busy={!!busy || restarting}
      width={620}
      footer={
        restarting ? undefined : (
          <>
            <Button onClick={close} disabled={!!busy}>
              취소
            </Button>
            {!header && (
              <Button variant="primary" icon={FolderOpen} onClick={pick} disabled={!!busy}>
                백업 파일 고르기
              </Button>
            )}
            {header && !preview && (
              <Button variant="primary" onClick={unlock} disabled={!!busy || pw.length === 0}>
                확인
              </Button>
            )}
            {preview && (
              <Button variant="danger" icon={RotateCw} onClick={confirm} disabled={!!busy || !agree}>
                복원하고 다시 시작
              </Button>
            )}
          </>
        )
      }
    >
      {restarting ? (
        <Notice tone="info">복원을 준비했습니다. 프로그램을 다시 시작합니다. 다시 열리면 복원한 자료를 한 번 더 확인한 결과가 대시보드에 보입니다.</Notice>
      ) : (
        <>
          {!header && (
            <p className={s.help}>
              이동용 백업 파일(.careercert-backup)을 고르세요. 고른 뒤 복구 비밀번호로 열어 내용을 확인하기 전까지 지금 자료는 바뀌지 않습니다.
            </p>
          )}
          {header && (
            <dl className={s.facts}>
              <dt>파일</dt>
              <dd>
                {header.fileName}{' '}
                {!preview && (
                  <Button size="sm" variant="ghost" onClick={pick} disabled={!!busy}>
                    다른 파일
                  </Button>
                )}
              </dd>
              <dt>백업 만든 시각</dt>
              <dd>{header.createdAtLabel}</dd>
              <dt>자료 구조 버전</dt>
              <dd>
                v{header.schemaVersion} <span className={s.sub}>(프로그램 {header.appVersion}에서 만듦)</span>
              </dd>
            </dl>
          )}
          {header && !preview && (
            <Field label="복구 비밀번호" hint="백업을 만들 때의 복구 비밀번호입니다.">
              <Input type="password" autoComplete="off" value={pw} onChange={(e) => setPw(e.target.value)} />
            </Field>
          )}
          {preview && <PreviewTable preview={preview} />}
          {preview && (
            <>
              <Notice tone="error">
                <b>현재 자료 전체가 선택한 백업의 자료로 교체됩니다.</b> 합치지 않습니다. 교체하기 직전에 지금 자료를 앱 안에 자동 백업(before_restore)해 둡니다.
              </Notice>
              <label className={s.agree}>
                <input type="checkbox" checked={agree} onChange={(e) => setAgree(e.target.checked)} /> 지금 자료가 위 백업의 자료로 바뀌는 것을 확인했습니다.
              </label>
            </>
          )}
          {busy && <Notice tone="info">{busy}</Notice>}
          <ErrorNotice error={error} />
        </>
      )}
    </Modal>
  )
}

function PreviewTable({ preview: p }: { preview: RestorePreview }) {
  return (
    <div className={s.preview}>
      <table className={s.table}>
        <thead>
          <tr>
            <th />
            <th className={s.num}>백업의 자료</th>
            <th className={s.num}>지금 자료 (바뀜)</th>
          </tr>
        </thead>
        <tbody>
          {COUNT_ROWS.map((r) => (
            <tr key={r.key}>
              <td>{r.label}</td>
              <td className={s.num}>{p.backup[r.key].toLocaleString()}</td>
              <td className={s.num}>{p.current[r.key].toLocaleString()}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <ul className={s.checks}>
        <li>
          <Badge tone="success">확인</Badge> 백업 인증·무결성·이 프로그램의 자료
        </li>
        <li>
          <Badge tone="success">확인</Badge> 발급 기록 {p.verifiedCertificates}건 모두 복호화·문서 지문 검증
        </li>
        <li>
          <Badge tone="info">{p.sameWindowsUser ? '같은 PC' : '다른 PC·계정'}</Badge>{' '}
          {p.sameWindowsUser
            ? '이 PC·이 Windows 계정에서 만든 백업입니다.'
            : '다른 PC 또는 다른 Windows 계정의 백업입니다. 복구 비밀번호로 키를 풀어 이 PC 의 Windows 계정으로 다시 보호합니다(발급 기록의 암호문은 그대로).'}
        </li>
        {p.migratedFrom < p.latestSchema && (
          <li>
            <Badge tone="info">구조</Badge> 자료 구조 v{p.migratedFrom} → v{p.latestSchema} 로 올려서 복원합니다.
          </li>
        )}
      </ul>
    </div>
  )
}
