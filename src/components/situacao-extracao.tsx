import { Badge } from '@/components/ui/badge'
import type { Metadados } from '@/lib/api'
import { plural } from '@/lib/formatar'

export function SituacaoExtracao({ metadados: m }: { metadados: Metadados }) {
  if (m.situacao === 'completa' && m.avisos.length === 0) {
    return <Badge className="border-transparent bg-emerald-500/12 text-emerald-700 dark:text-emerald-300">Completa</Badge>
  }
  if (m.situacao === 'completa') {
    return (
      <Badge className="border-transparent bg-amber-500/15 text-amber-700 dark:text-amber-300">
        Completa com {plural(m.avisos.length, 'aviso')}
      </Badge>
    )
  }
  if (m.situacao === 'cancelada') return <Badge variant="outline">Cancelada (parcial)</Badge>
  return (
    <Badge variant="destructive" title={m.erro ?? undefined}>
      Parou com erro (parcial)
    </Badge>
  )
}
