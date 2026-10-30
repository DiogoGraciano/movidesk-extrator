import { Badge } from '@/components/ui/badge'
import { nomesStatusBase } from '@/lib/formatar'
import { cn } from '@/lib/utils'

const cores: Record<string, string> = {
  New: 'bg-sky-500/12 text-sky-700 dark:text-sky-300',
  InAttendance: 'bg-violet-500/12 text-violet-700 dark:text-violet-300',
  Stopped: 'bg-amber-500/15 text-amber-700 dark:text-amber-300',
  Resolved: 'bg-emerald-500/12 text-emerald-700 dark:text-emerald-300',
  Closed: 'bg-muted text-muted-foreground',
  Canceled: 'bg-rose-500/12 text-rose-700 dark:text-rose-300',
}

/** Status do ticket: o texto configurado na conta, colorido pelo status base. */
export function StatusTicket({
  status,
  statusBase,
  className,
}: {
  status: string | null | undefined
  statusBase: string | null | undefined
  className?: string
}) {
  const texto = status ?? (statusBase ? nomesStatusBase[statusBase] : null)
  if (!texto) return null
  return (
    <Badge
      variant="secondary"
      className={cn('border-transparent', statusBase && cores[statusBase], className)}
      title={statusBase ? nomesStatusBase[statusBase] : undefined}
    >
      {texto}
    </Badge>
  )
}
