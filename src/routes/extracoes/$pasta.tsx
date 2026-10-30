import { useMutation, useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute, Link, Outlet } from '@tanstack/react-router'
import { ArrowLeftIcon, FolderOpenIcon, PaperclipIcon, RefreshCwIcon, SearchIcon } from 'lucide-react'
import { useDeferredValue, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { SituacaoExtracao } from '@/components/situacao-extracao'
import { StatusTicket } from '@/components/status-ticket'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { InputGroup, InputGroupAddon, InputGroupInput } from '@/components/ui/input-group'
import { Spinner } from '@/components/ui/spinner'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { api, mensagemDeErro, type TicketResumo } from '@/lib/api'
import { extracaoQuery } from '@/lib/consultas'
import { lerConfiguracoes, useConfiguracoes } from '@/lib/configuracoes'
import { dataHora, dia, plural } from '@/lib/formatar'
import { useProgresso } from '@/lib/progresso'
import { cn } from '@/lib/utils'

export const Route = createFileRoute('/extracoes/$pasta')({
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(extracaoQuery(lerConfiguracoes().pastaBase, params.pasta)),
  pendingComponent: () => (
    <div className="flex h-full items-center justify-center">
      <Spinner className="size-6 text-muted-foreground" />
    </div>
  ),
  errorComponent: ({ error }) => (
    <div className="mx-auto max-w-xl p-6">
      <Alert variant="destructive">
        <AlertTitle>Não foi possível abrir a extração</AlertTitle>
        <AlertDescription>{mensagemDeErro(error)}</AlertDescription>
      </Alert>
    </div>
  ),
  component: PaginaExtracao,
})

type Filtro = 'todos' | 'abertos' | 'resolvidos' | 'fechados' | 'cancelados'
const filtros: { valor: Filtro; rotulo: string; bases?: string[] }[] = [
  { valor: 'todos', rotulo: 'Todos' },
  { valor: 'abertos', rotulo: 'Abertos', bases: ['New', 'InAttendance', 'Stopped'] },
  { valor: 'resolvidos', rotulo: 'Resolvidos', bases: ['Resolved'] },
  { valor: 'fechados', rotulo: 'Fechados', bases: ['Closed'] },
  { valor: 'cancelados', rotulo: 'Cancelados', bases: ['Canceled'] },
]

function normalizar(texto: string) {
  return texto
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()
}

function textoDeBusca(t: TicketResumo) {
  return normalizar(
    [t.id, t.protocolo, t.assunto, t.status, t.categoria, t.servico, t.responsavel, t.equipe, ...t.clientes]
      .filter(Boolean)
      .join(' '),
  )
}

function PaginaExtracao() {
  const { pasta } = Route.useParams()
  const { pastaBase } = useConfiguracoes()
  const { data: extracao } = useSuspenseQuery(extracaoQuery(pastaBase, pasta))
  const progresso = useProgresso()
  const [termo, setTermo] = useState('')
  const [filtro, setFiltro] = useState<Filtro>('todos')
  const termoAdiado = useDeferredValue(termo)
  const m = extracao.metadados
  const extraindoEste = progresso?.estado === 'rodando' && progresso.pasta === pasta

  const indice = useMemo(
    () => extracao.tickets.map((t) => ({ ticket: t, texto: textoDeBusca(t) })),
    [extracao.tickets],
  )
  const visiveis = useMemo(() => {
    const partes = normalizar(termoAdiado).split(/\s+/).filter(Boolean)
    const bases = filtros.find((f) => f.valor === filtro)?.bases
    return indice
      .filter(({ ticket, texto }) => {
        if (bases && !bases.includes(ticket.statusBase ?? '')) return false
        return partes.every((p) => texto.includes(p))
      })
      .map(({ ticket }) => ticket)
  }, [indice, termoAdiado, filtro])

  const abrirPasta = useMutation({
    mutationFn: () => api.abrirCaminho(pastaBase, pasta),
    onError: (e) => toast.error(mensagemDeErro(e)),
  })

  return (
    <div className="flex h-full">
      <aside className="flex w-[400px] shrink-0 flex-col border-r bg-card">
        <div className="space-y-3 border-b p-4">
          <Link
            to="/extracoes"
            className="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
          >
            <ArrowLeftIcon className="size-3.5" />
            Extrações
          </Link>
          <div className="space-y-1">
            <h1 className="truncate text-lg font-semibold tracking-tight" title={m.cliente.nome}>
              {m.cliente.nome}
            </h1>
            <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
              <SituacaoExtracao metadados={m} />
              <span>{plural(m.ticketsSalvos, 'ticket')}</span>
              <span>·</span>
              <span>{dataHora(m.concluidaEm)}</span>
            </div>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={() => abrirPasta.mutate()}>
              <FolderOpenIcon />
              Abrir pasta
            </Button>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="outline" size="sm" asChild disabled={extraindoEste}>
                  <Link
                    to="/"
                    search={{ cliente: m.cliente.id, nome: m.cliente.nome, tipo: m.cliente.tipoPessoa }}
                  >
                    <RefreshCwIcon />
                    Extrair de novo
                  </Link>
                </Button>
              </TooltipTrigger>
              <TooltipContent className="max-w-64">
                Busca tickets novos e atualizados; os que não mudaram são lidos do disco.
              </TooltipContent>
            </Tooltip>
          </div>
          {extraindoEste && (
            <p className="flex items-center gap-2 text-xs text-primary">
              <Spinner className="size-3.5" />
              Extração em andamento; a lista atualiza ao terminar.
            </p>
          )}
        </div>

        <div className="space-y-2 border-b p-3">
          <InputGroup>
            <InputGroupAddon>
              <SearchIcon />
            </InputGroupAddon>
            <InputGroupInput
              placeholder="Filtrar por número, assunto, pessoa…"
              value={termo}
              onChange={(e) => setTermo(e.target.value)}
            />
          </InputGroup>
          <div className="flex flex-wrap gap-1">
            {filtros.map((f) => (
              <button
                key={f.valor}
                type="button"
                onClick={() => setFiltro(f.valor)}
                aria-pressed={filtro === f.valor}
                className={cn(
                  'rounded-full border px-2.5 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-muted',
                  filtro === f.valor && 'border-primary/40 bg-accent text-accent-foreground',
                )}
              >
                {f.rotulo}
              </button>
            ))}
          </div>
        </div>

        <ul className="min-h-0 flex-1 overflow-y-auto" aria-label="Tickets">
          {visiveis.map((t) => (
            <li key={t.id}>
              <ItemTicket ticket={t} pasta={pasta} />
            </li>
          ))}
          {visiveis.length === 0 && (
            <li className="p-6 text-center text-sm text-muted-foreground">Nenhum ticket com esse filtro.</li>
          )}
        </ul>
        <div className="border-t px-4 py-2 text-xs text-muted-foreground">
          {visiveis.length === extracao.tickets.length
            ? plural(visiveis.length, 'ticket')
            : `${visiveis.length.toLocaleString('pt-BR')} de ${plural(extracao.tickets.length, 'ticket')}`}
        </div>
      </aside>

      <section className="min-w-0 flex-1">
        <Outlet />
      </section>
    </div>
  )
}

function ItemTicket({ ticket: t, pasta }: { ticket: TicketResumo; pasta: string }) {
  return (
    <Link
      to="/extracoes/$pasta/$ticketId"
      params={{ pasta, ticketId: String(t.id) }}
      className="block space-y-1 border-b px-4 py-3 transition-colors hover:bg-muted/60 data-[status=active]:bg-accent"
    >
      <div className="flex items-center gap-2">
        <span className="font-mono text-xs text-muted-foreground">#{t.id}</span>
        <StatusTicket status={t.status} statusBase={t.statusBase} className="h-5 px-1.5 text-[11px]" />
        {t.qtdAnexos > 0 && <PaperclipIcon className="size-3 text-muted-foreground" aria-label="Tem anexos" />}
        <span className="ml-auto text-xs text-muted-foreground tabular-nums">{dia(t.criadoEm)}</span>
      </div>
      <p className="line-clamp-2 text-sm font-medium leading-snug">{t.assunto}</p>
      <p className="truncate text-xs text-muted-foreground">
        {[t.clientes.join(', '), t.responsavel].filter(Boolean).join(' → ')}
      </p>
    </Link>
  )
}
