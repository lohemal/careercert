import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'

import { Button, ErrorNotice, Field, Input, Modal, Textarea } from '@/components/ui'
import { instructorApi, type Instructor, type InstructorInput } from '@/ipc/instructor'
import { invalidateCareerData } from './invalidate'
import s from './dialogs.module.css'

interface Props {
  /** 없으면 새 강사 */
  instructor?: Instructor
  onClose: () => void
  onSaved: (saved: Instructor) => void
}

/**
 * 강사 등록·수정. 주민등록번호·주소 칸은 **만들지 않는다** — 발급할 때만 입력하고 발급 기록에만 남는다(결정 D3).
 * 검사는 Rust `domain::instructor::prepare` 가 한다.
 */
export function InstructorDialog({ instructor, onClose, onSaved }: Props) {
  const qc = useQueryClient()
  const [form, setForm] = useState<InstructorInput>({
    name: instructor?.name ?? '',
    distinguisher: instructor?.distinguisher ?? '',
    phone: instructor?.phone ?? '',
    memo: instructor?.memo ?? '',
  })
  const save = useMutation({
    mutationFn: () => (instructor ? instructorApi.update(instructor.id, form) : instructorApi.create(form)),
    onSuccess: (saved) => {
      invalidateCareerData(qc)
      onSaved(saved)
    },
  })
  const set = (k: keyof InstructorInput, v: string) => {
    setForm({ ...form, [k]: v })
    save.reset()
  }

  return (
    <Modal
      title={instructor ? '강사 정보 수정' : '강사 추가'}
      onClose={onClose}
      busy={save.isPending}
      footer={
        <>
          <Button onClick={onClose} disabled={save.isPending}>
            취소
          </Button>
          <Button variant="primary" onClick={() => save.mutate()} disabled={save.isPending || !form.name.trim()}>
            {save.isPending ? '저장 중…' : '저장'}
          </Button>
        </>
      }
    >
      <ErrorNotice error={save.error} />
      <form
        className={s.form}
        onSubmit={(e) => {
          e.preventDefault()
          if (form.name.trim()) save.mutate()
        }}
      >
        <Field label="성명">
          <Input value={form.name} onChange={(e) => set('name', e.target.value)} autoFocus placeholder="예: 김○○" />
        </Field>
        <Field
          label="동명이인 구분 메모"
          hint="같은 이름의 강사가 있으면 꼭 적어 주세요. 발급할 때 이 메모로 사람을 가립니다. 예: 1985년생, 마술 강사, 이전 계약자"
        >
          <Input value={form.distinguisher} onChange={(e) => set('distinguisher', e.target.value)} />
        </Field>
        <Field label="전화번호">
          <Input value={form.phone} onChange={(e) => set('phone', e.target.value)} placeholder="선택" />
        </Field>
        <Field label="메모">
          <Textarea value={form.memo} rows={3} onChange={(e) => set('memo', e.target.value)} placeholder="선택" />
        </Field>
        <button type="submit" hidden />
      </form>
      <p className={s.note}>주민등록번호·주소는 여기에 저장하지 않습니다. 증명서를 발급할 때만 입력합니다.</p>
    </Modal>
  )
}
