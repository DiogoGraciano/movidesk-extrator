import { useMutation, useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { FolderOpenIcon, InboxIcon, RefreshCwIcon } from 'lucide-react'
import { toast } from 'sonner'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty'
import { Skeleton } from '@/components/ui/skeleton'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { SituacaoExtracao } from '@/components/situacao-extracao'
import { api, mensagemDeErro } from '@/lib/api'
import { extracoesQuery } from '@/lib/consultas'
import { lerConfiguracoes, useConfiguracoes } from '@/lib/configuracoes'
import { dataHora, plural, tipoPessoa } from '@/lib/formatar'

export const Route = createFileRoute('/extracoes/')({
  loader: ({ context }) => {
    const { pastaBase } = lerConfiguracoes()
    if (pastaBase) void context.queryClient.prefetchQuery(extracoesQuery(pastaBase))
  },
  component: PaginaExtracoes,
})

function PaginaExtracoes() {
  const { pastaBase } = useConfiguracoes()
  const extracoes = useQuery(extracoesQuery(pastaBase))
  const abrir = useMutation({
    mutationFn: (pasta: string) => api.abrirCaminho(pastaBase, pasta),
    onError: (e) => toast.error(mensagemDeErro(e)),
  })

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-6xl space-y-5 p-6">
        <div className="flex items-end justify-between gap-4">
          <div className="min-w-0 space-y-1">
            <h1 className="text-xl font-semibold tracking-tight">Extrações salvas</h1>
            <p className="truncate text-sm text-muted-foreground" title={pastaBase}>
              Em {pastaBase || 'nenhuma pasta escolhida'}
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={() => extracoes.refetch()} disabled={extracoes.isFetching}>
            <RefreshCwIcon className={extracoes.isFetching ? 'animate-spin' : undefined} />
            Atualizar
          </Button>
        </div>

        {extracoes.isPending ? (
          <Skeleton className="h-48 w-full" />
        ) : extracoes.isError ? (
          <Alert variant="destructive">
            <AlertDescription>{mensagemDeErro(extracoes.error)}</AlertDescription>
          </Alert>
        ) : extracoes.data.length === 0 ? (
          <Empty className="border">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <InboxIcon />
              </EmptyMedia>
              <EmptyTitle>Nenhuma extração ainda</EmptyTitle>
              <EmptyDescription>As extrações feitas aparecem aqui, lidas direto da pasta de destino.</EmptyDescription>
            </EmptyHeader>
            <EmptyContent>
              <Button asChild>
                <Link to="/">Extrair tickets de um cliente</Link>
              </Button>
            </EmptyContent>
          </Empty>
        ) : (
          <div className="overflow-hidden rounded-lg border bg-card">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Cliente</TableHead>
                  <TableHead className="text-right">Tickets</TableHead>
                  <TableHead>Situação</TableHead>
                  <TableHead>Extraída em</TableHead>
                  <TableHead className="w-0" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {extracoes.data.map(({ pasta, metadados: m }) => (
                  <TableRow key={pasta}>
                    <TableCell>
                      <Link
                        to="/extracoes/$pasta"
                        params={{ pasta }}
                        className="font-medium hover:text-primary hover:underline"
                      >
                        {m.cliente.nome}
                      </Link>
                      <div className="text-xs text-muted-foreground">
                        {tipoPessoa(m.cliente.tipoPessoa)} · código {m.cliente.id}
                        {m.membros.length > 0 && ` · ${plural(m.membros.length, 'pessoa vinculada', 'pessoas vinculadas')}`}
                      </div>
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {m.ticketsSalvos.toLocaleString('pt-BR')}
                      {m.ticketsSalvos < m.ticketsListados && (
                        <span className="text-muted-foreground"> / {m.ticketsListados.toLocaleString('pt-BR')}</span>
                      )}
                    </TableCell>
                    <TableCell>
                      <SituacaoExtracao metadados={m} />
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">{dataHora(m.concluidaEm)}</TableCell>
                    <TableCell>
                      <Button variant="ghost" size="icon-sm" aria-label="Abrir pasta" onClick={() => abrir.mutate(pasta)}>
                        <FolderOpenIcon />
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </div>
    </div>
  )
}
