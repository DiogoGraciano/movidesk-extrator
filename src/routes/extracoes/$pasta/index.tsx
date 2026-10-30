import { createFileRoute } from '@tanstack/react-router'
import { MessagesSquareIcon } from 'lucide-react'
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty'

export const Route = createFileRoute('/extracoes/$pasta/')({
  component: () => (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <MessagesSquareIcon />
        </EmptyMedia>
        <EmptyTitle>Escolha um ticket</EmptyTitle>
        <EmptyDescription>A conversa completa aparece aqui, ação por ação.</EmptyDescription>
      </EmptyHeader>
    </Empty>
  ),
})
