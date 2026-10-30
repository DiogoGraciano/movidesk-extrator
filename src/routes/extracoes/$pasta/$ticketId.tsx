import { useMutation, useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { LockIcon, PaperclipIcon } from 'lucide-react'
import { useId, useState } from 'react'
import { toast } from 'sonner'
import { StatusTicket } from '@/components/status-ticket'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { type AcaoMovidesk, type AnexoLocal, api, mensagemDeErro, type PessoaMovidesk } from '@/lib/api'
import { ticketQuery } from '@/lib/consultas'
import { lerConfiguracoes, useConfiguracoes } from '@/lib/configuracoes'
import { dataHora, nomesStatusBase, origem, plural } from '@/lib/formatar'
import { cn } from '@/lib/utils'

export const Route = createFileRoute('/extracoes/$pasta/$ticketId')({
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(
      ticketQuery(lerConfiguracoes().pastaBase, params.pasta, Number(params.ticketId)),
    ),
  pendingComponent: () => (
    <div className="flex h-full items-center justify-center">
      <Spinner className="size-6 text-muted-foreground" />
    </div>
  ),
  errorComponent: ({ error }) => (
    <div className="p-6">
      <Alert variant="destructive">
        <AlertDescription>{mensagemDeErro(error)}</AlertDescription>
      </Alert>
    </div>
  ),
  component: PaginaTicket,
})

function nome(p: PessoaMovidesk | null | undefined) {
  return p?.businessName?.trim() || null
}

function servico(valor: string[] | string | null | undefined) {
  if (Array.isArray(valor)) return valor.filter(Boolean).join(' › ') || null
  return valor || null
}

function PaginaTicket() {
  const { pasta, ticketId } = Route.useParams()
  const { pastaBase } = useConfiguracoes()
  const { data } = useSuspenseQuery(ticketQuery(pastaBase, pasta, Number(ticketId)))
  const { ticket: t, anexos } = data
  const [mostrarInternas, setMostrarInternas] = useState(true)
  const idInternas = useId()

  const acoes = (t.actions ?? []).filter((a) => !a.isDeleted)
  const internas = acoes.filter((a) => a.type === 1).length
  const visiveis = mostrarInternas ? acoes : acoes.filter((a) => a.type !== 1)
  const clientes = (t.clients ?? []).map((c) => {
    const org = nome(c.organization)
    const n = nome(c) ?? c.id
    return org && org !== n ? `${n} (${org})` : n
  })

  const campos: [string, string | null][] = [
    ['Protocolo', t.protocol ?? null],
    ['Cliente(s)', clientes.join(', ') || null],
    ['Responsável', [nome(t.owner), t.ownerTeam].filter(Boolean).join(' · ') || null],
    ['Categoria', t.category ?? null],
    ['Urgência', t.urgency ?? null],
    ['Serviço', servico(t.serviceFull)],
    ['Origem', origem(t.origin)],
    ['Aberto em', t.createdDate ? dataHora(t.createdDate) : null],
    ['Resolvido em', t.resolvedIn ? dataHora(t.resolvedIn) : null],
    ['Fechado em', t.closedIn ? dataHora(t.closedIn) : null],
    ['Última atualização', t.lastUpdate ? dataHora(t.lastUpdate) : null],
  ]

  return (
    <div className="h-full overflow-y-auto" key={t.id}>
      <div className="mx-auto max-w-4xl space-y-6 p-6">
        <header className="space-y-4">
          <div className="space-y-2">
            <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
              <span className="font-mono">#{t.id}</span>
              <StatusTicket status={t.status} statusBase={t.baseStatus} />
              {t.baseStatus && t.status && nomesStatusBase[t.baseStatus] !== t.status && (
                <span className="text-xs">({nomesStatusBase[t.baseStatus]})</span>
              )}
            </div>
            <h2 className="text-xl font-semibold leading-tight tracking-tight">{t.subject || '(sem assunto)'}</h2>
            {t.tags && t.tags.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {t.tags.map((tag) => (
                  <Badge key={tag} variant="outline" className="font-normal">
                    {tag}
                  </Badge>
                ))}
              </div>
            )}
          </div>
          <dl className="grid grid-cols-1 gap-x-6 gap-y-2 rounded-lg border bg-card p-4 text-sm sm:grid-cols-2">
            {campos
              .filter(([, valor]) => valor)
              .map(([rotulo, valor]) => (
                <div key={rotulo} className="flex min-w-0 gap-2">
                  <dt className="shrink-0 text-muted-foreground">{rotulo}:</dt>
                  <dd className="min-w-0 break-words">{valor}</dd>
                </div>
              ))}
          </dl>
        </header>

        <section className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <h3 className="font-semibold">
              Conversa <span className="font-normal text-muted-foreground">({plural(acoes.length, 'ação', 'ações')})</span>
            </h3>
            {internas > 0 && (
              <div className="flex items-center gap-2">
                <Checkbox
                  id={idInternas}
                  checked={mostrarInternas}
                  onCheckedChange={(v) => setMostrarInternas(v === true)}
                />
                <Label htmlFor={idInternas} className="font-normal text-muted-foreground">
                  Mostrar {internas} {internas === 1 ? 'ação interna' : 'ações internas'}
                </Label>
              </div>
            )}
          </div>
          <ol className="space-y-3">
            {visiveis.map((acao) => (
              <li key={acao.id}>
                <Acao acao={acao} anexos={anexos.filter((a) => a.acaoId === acao.id)} pastaBase={pastaBase} />
              </li>
            ))}
          </ol>
        </section>
      </div>
    </div>
  )
}

function iniciais(texto: string) {
  const partes = texto.split(/\s+/).filter(Boolean)
  return ((partes[0]?.[0] ?? '?') + (partes.length > 1 ? (partes.at(-1)?.[0] ?? '') : '')).toUpperCase()
}

function Acao({ acao, anexos, pastaBase }: { acao: AcaoMovidesk; anexos: AnexoLocal[]; pastaBase: string }) {
  const autor = nome(acao.createdBy)
  const interna = acao.type === 1
  // Perfil 1 = agente, 3 = agente e cliente; sem autor é ação do sistema.
  const doAgente = acao.createdBy?.profileType === 1 || acao.createdBy?.profileType === 3
  const doCliente = acao.createdBy?.profileType === 2
  // Transcrição de chat separa as mensagens com "\n\r\n" (LF + CRLF); vira uma
  // quebra só. O lookbehind evita pegar o parágrafo de e-mail ("\r\n\r\n").
  const texto = (acao.description ?? '')
    .replace(/(?<!\r)\n\r\n/g, '\n')
    .replace(/\r\n?/g, '\n')
    .trim()
  const abrir = useMutation({
    mutationFn: (relativo: string) => api.abrirCaminho(pastaBase, relativo),
    onError: (e) => toast.error(mensagemDeErro(e)),
  })

  return (
    <article
      className={cn(
        'rounded-lg border bg-card',
        doCliente && 'border-l-4 border-l-primary',
        doAgente && !interna && 'border-l-4 border-l-muted-foreground/25',
        interna && 'border-dashed border-amber-500/50 bg-amber-500/5',
      )}
    >
      <header className="flex items-center gap-3 border-b px-4 py-2.5">
        <span
          className={cn(
            'flex size-8 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-semibold text-muted-foreground',
            doCliente && 'bg-accent text-accent-foreground',
          )}
          aria-hidden
        >
          {autor ? iniciais(autor) : '⚙'}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5">
            <span className="truncate text-sm font-medium">{autor ?? 'Sistema'}</span>
            {doCliente && <span className="text-xs font-medium text-primary">cliente</span>}
            {interna && (
              <Badge variant="outline" className="gap-1 border-amber-500/50 text-amber-700 dark:text-amber-300">
                <LockIcon className="size-3" />
                Interna
              </Badge>
            )}
          </div>
          <div className="text-xs text-muted-foreground">
            {[origem(acao.origin), acao.status && `Status: ${acao.status}${acao.justification ? ` (${acao.justification})` : ''}`]
              .filter(Boolean)
              .join(' · ')}
          </div>
        </div>
        <div className="shrink-0 text-right text-xs text-muted-foreground tabular-nums">
          <div>{dataHora(acao.createdDate)}</div>
          <div>ação {acao.id}</div>
        </div>
      </header>
      <div className="px-4 py-3">
        {texto ? (
          <p className="whitespace-pre-wrap break-words text-sm leading-relaxed">{texto}</p>
        ) : (
          <p className="text-sm italic text-muted-foreground">Sem texto.</p>
        )}
        {anexos.length > 0 && (
          <ul className="mt-3 flex flex-wrap gap-2">
            {anexos.map((anexo) => (
              <li key={`${anexo.hash}-${anexo.nome}`}>
                {anexo.caminhoRelativo ? (
                  <button
                    type="button"
                    onClick={() => anexo.caminhoRelativo && abrir.mutate(anexo.caminhoRelativo)}
                    className="inline-flex max-w-72 items-center gap-1.5 rounded-md border bg-background px-2 py-1 text-xs hover:bg-muted"
                  >
                    <PaperclipIcon className="size-3 shrink-0" />
                    <span className="truncate">{anexo.nome}</span>
                  </button>
                ) : (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="inline-flex max-w-72 cursor-default items-center gap-1.5 rounded-md border border-dashed px-2 py-1 text-xs text-muted-foreground">
                        <PaperclipIcon className="size-3 shrink-0" />
                        <span className="truncate">{anexo.nome}</span>
                      </span>
                    </TooltipTrigger>
                    <TooltipContent>
                      Não baixado. Ligue “Baixar também os anexos” e extraia de novo.
                    </TooltipContent>
                  </Tooltip>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
    </article>
  )
}
