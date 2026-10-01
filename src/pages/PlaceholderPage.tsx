import { Badge, Card, Page } from '@/components/ui'

interface Props {
  title: string
  /** 어느 Phase 에서 만드는가 */
  phase: string
  description: string
}

/** 메뉴 자리만 잡아 둔 빈 화면 */
export function PlaceholderPage({ title, phase, description }: Props) {
  return (
    <Page title={title} badge={<Badge tone="info">{phase} 에서 구현</Badge>}>
      <Card>
        <p>{description}</p>
      </Card>
    </Page>
  )
}
