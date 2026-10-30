import { useMutation } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import {
  CheckIcon,
  ChevronDownIcon,
  FolderOpenIcon,
  HourglassIcon,
  ListIcon,
  OctagonXIcon,
  TriangleAlertIcon,
} from 'lucide-react'
import { toast } from 'sonner'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
import { api, type FaseExtracao, mensagemDeErro, type Progresso } from '@/lib/api'
import { useConfiguracoes } from '@/lib/configuracoes'
import { duracao, plural, tipoPessoa } from '@/lib/formatar'
import { useContagem } from '@/lib/progresso'
import { cn } from '@/lib/utils'

/** Progresso da fase atual, de 0 a 100. */
export function percentual(p: Progresso) {
  if (p.fase === 'tickets' && p.ticketsTotal > 0) return (p.ticketsProcessados / p.ticketsTotal) * 100
  if (p.fase === 'anexos' && p.anexosTotal > 0) return (p.anexosProcessados / p.anexosTotal) * 100
  if (p.fase === 'arquivos' || p.fase === 'fim') return 100
  return p.fase === 'tickets' ? 100 : 0
}

const etapas: { fase: FaseExtracao; rotulo: string }[] = [
  { fase: 'cliente', rotulo: 'Cliente' },
  { fase: 'listagem', rotulo: 'Listagem' },
  { fase: 'tickets', rotulo: 'Conversas' },
  { fase: 'anexos', rotulo: 'Anexos' },
  { fase: 'arquivos', rotulo: 'Arquivos' },
]
const ordem: FaseExtracao[] = ['cliente', 'listagem', 'tickets', 'anexos', 'arquivos', 'fim']

export function ProgressoExtracao({ progresso: p }: { progresso: Progresso }) {
  const { pastaBase } = useConfiguracoes()
  const rodando = p.estado === 'rodando'
  const espera = useContagem(rodando ? p.esperaSegundos : null, p.atualizadaEm)
  const restante = useContagem(rodando ? p.restanteSegundos : null, p.atualizadaEm)
  const cancelar = useMutation({
    mutationFn: api.cancelarExtracao,
    onError: (e) => toast.error(mensagemDeErro(e)),
  })
  const abrirPasta = useMutation({
    mutationFn: (pasta: string) => api.abrirCaminho(pastaBase, pasta),
    onError: (e) => toast.error(mensagemDeErro(e)),
  })
  const comAnexos = p.anexosTotal > 0 || p.fase === 'anexos'
  const indiceAtual = ordem.indexOf(p.fase)

  return (
    <Card
      className={cn(
        'gap-5 overflow-hidden',
        p.estado === 'concluida' && 'border-success/40',
        p.estado === 'erro' && 'border-destructive/40',
      )}
    >
      <CardHeader className="flex flex-row items-start justify-between gap-4">
        <div className="min-w-0 space-y-1.5">
          <CardTitle className="flex items-center gap-2 text-base">
            {rodando ? <Spinner className="size-4 text-primary" /> : <IconeDoEstado estado={p.estado} />}
            <span className="truncate">
              {rodando ? 'Extraindo' : 'Extração de'} {p.cliente?.nome ?? 'cliente'}
            </span>
            {p.cliente && <Badge variant="secondary">{tipoPessoa(p.cliente.tipoPessoa)}</Badge>}
          </CardTitle>
          <CardDescription className="truncate">{p.erro ?? p.mensagem}</CardDescription>
        </div>
        <div className="flex shrink-0 gap-2">
          {rodando ? (
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button variant="outline" size="sm" disabled={cancelar.isPending}>
                  Cancelar
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Cancelar a extração?</AlertDialogTitle>
                  <AlertDialogDescription>
                    Os tickets já baixados ficam salvos na pasta. Ao extrair o mesmo cliente de novo, o app
                    continua de onde parou sem baixá-los outra vez.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Continuar extraindo</AlertDialogCancel>
                  <AlertDialogAction onClick={() => cancelar.mutate()}>Cancelar extração</AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          ) : (
            p.pasta && (
              <>
                <Button variant="outline" size="sm" onClick={() => p.pasta && abrirPasta.mutate(p.pasta)}>
                  <FolderOpenIcon />
                  Abrir pasta
                </Button>
                <Button size="sm" asChild>
                  <Link to="/extracoes/$pasta" params={{ pasta: p.pasta }}>
                    <ListIcon />
                    Ver tickets
                  </Link>
                </Button>
              </>
            )
          )}
        </div>
      </CardHeader>

      <CardContent className="space-y-5">
        <ol className="flex items-center gap-2 text-xs">
          {etapas
            .filter((e) => e.fase !== 'anexos' || comAnexos)
            .map((etapa, i) => {
              const indice = ordem.indexOf(etapa.fase)
              const feita = indice < indiceAtual || p.fase === 'fim'
              const atual = indice === indiceAtual && rodando
              return (
                <li key={etapa.fase} className="flex items-center gap-2">
                  {i > 0 && <span className="h-px w-6 bg-border" />}
                  <span
                    className={cn(
                      'flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-muted-foreground',
                      feita && 'border-transparent bg-muted text-foreground',
                      atual && 'border-primary/40 bg-accent font-medium text-accent-foreground',
                    )}
                  >
                    {feita && <CheckIcon className="size-3" />}
                    {etapa.rotulo}
                  </span>
                </li>
              )
            })}
        </ol>

        {p.ticketsTotal > 0 && (
          <BarraDeFase
            rotulo={`${p.ticketsProcessados.toLocaleString('pt-BR')} de ${plural(p.ticketsTotal, 'ticket')}`}
            valor={(p.ticketsProcessados / p.ticketsTotal) * 100}
            restante={p.fase === 'tickets' ? restante : null}
          />
        )}
        {comAnexos && p.anexosTotal > 0 && (
          <BarraDeFase
            rotulo={`${p.anexosProcessados.toLocaleString('pt-BR')} de ${plural(p.anexosTotal, 'anexo')}`}
            valor={(p.anexosProcessados / p.anexosTotal) * 100}
            restante={p.fase === 'anexos' ? restante : null}
          />
        )}

        <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <Numero rotulo="Tickets encontrados" valor={p.ticketsTotal || p.ticketsListados} />
          <Numero rotulo="Baixados agora" valor={p.ticketsBaixados} />
          <Numero rotulo="Reaproveitados do disco" valor={p.ticketsReaproveitados} />
          <Numero
            rotulo={`Requisições (limite ${p.requisicoesPorMinuto}/min)`}
            valor={p.requisicoes}
          />
        </dl>

        {espera != null && espera > 0 && (
          <p className="flex items-center gap-2 rounded-md bg-muted px-3 py-2 text-sm text-muted-foreground">
            <HourglassIcon className="size-4 shrink-0" />
            Aguardando a API liberar a próxima requisição: {duracao(espera)}.
          </p>
        )}

        {p.avisos.length > 0 && (
          <Collapsible>
            <CollapsibleTrigger className="group flex items-center gap-2 text-sm font-medium text-warning">
              <TriangleAlertIcon className="size-4" />
              {plural(p.avisos.length, 'aviso')}
              <ChevronDownIcon className="size-4 transition-transform group-data-[state=open]:rotate-180" />
            </CollapsibleTrigger>
            <CollapsibleContent>
              <ul className="mt-2 max-h-40 space-y-1 overflow-y-auto rounded-md border bg-muted/40 p-3 text-xs text-muted-foreground">
                {p.avisos.map((aviso, i) => (
                  <li key={`${i}-${aviso}`}>{aviso}</li>
                ))}
              </ul>
            </CollapsibleContent>
          </Collapsible>
        )}

        {p.estado === 'erro' && p.erro && (
          <Alert variant="destructive">
            <OctagonXIcon />
            <AlertTitle>A extração parou</AlertTitle>
            <AlertDescription>
              {p.erro} Os tickets já baixados foram salvos; extraia de novo para continuar.
            </AlertDescription>
          </Alert>
        )}
      </CardContent>
    </Card>
  )
}

function IconeDoEstado({ estado }: { estado: Progresso['estado'] }) {
  if (estado === 'concluida') return <CheckIcon className="size-4 text-success" />
  if (estado === 'erro') return <OctagonXIcon className="size-4 text-destructive" />
  return <TriangleAlertIcon className="size-4 text-warning" />
}

function BarraDeFase({ rotulo, valor, restante }: { rotulo: string; valor: number; restante: number | null }) {
  return (
    <div className="space-y-2">
      <div className="flex items-baseline justify-between text-sm">
        <span className="font-medium tabular-nums">{rotulo}</span>
        {restante != null && restante > 0 && (
          <span className="text-muted-foreground tabular-nums">≈ {duracao(restante)} restantes</span>
        )}
      </div>
      <Progress value={valor} className="h-2" />
    </div>
  )
}

function Numero({ rotulo, valor }: { rotulo: string; valor: number }) {
  return (
    <div className="rounded-lg border bg-muted/30 px-3 py-2.5">
      <dt className="text-xs text-muted-foreground">{rotulo}</dt>
      <dd className="text-lg font-semibold tabular-nums">{valor.toLocaleString('pt-BR')}</dd>
    </div>
  )
}
