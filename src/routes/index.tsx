import { useMutation } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { open } from '@tauri-apps/plugin-dialog'
import { DownloadIcon, FolderIcon, SlidersHorizontalIcon } from 'lucide-react'
import { useId } from 'react'
import { toast } from 'sonner'
import { CartaoChave } from '@/components/cartao-chave'
import { CartaoCliente, type ClienteEscolhido } from '@/components/cartao-cliente'
import { ProgressoExtracao } from '@/components/progresso-extracao'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Field, FieldContent, FieldDescription, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group'
import { Spinner } from '@/components/ui/spinner'
import { api, mensagemDeErro } from '@/lib/api'
import { alterarConfiguracoes, REQUISICOES_POR_MINUTO_PADRAO, useConfiguracoes } from '@/lib/configuracoes'
import { definirProgresso, useProgresso } from '@/lib/progresso'

type Busca = { cliente?: string; nome?: string; tipo?: number }

// O cliente escolhido fica na URL: assim "Extrair de novo", na tela de uma
// extração salva, só precisa navegar para cá com ele preenchido.
export const Route = createFileRoute('/')({
  validateSearch: (busca: Record<string, unknown>): Busca => ({
    cliente: typeof busca.cliente === 'string' ? busca.cliente : undefined,
    nome: typeof busca.nome === 'string' ? busca.nome : undefined,
    tipo: typeof busca.tipo === 'number' ? busca.tipo : undefined,
  }),
  component: PaginaExtrair,
})

function PaginaExtrair() {
  const busca = Route.useSearch()
  const navigate = Route.useNavigate()
  const config = useConfiguracoes()
  const progresso = useProgresso()
  const rodando = progresso?.estado === 'rodando'

  const escolhido: ClienteEscolhido | null = busca.cliente
    ? { id: busca.cliente, nome: busca.nome ?? busca.cliente, tipo: busca.tipo ?? 1 }
    : null

  const iniciar = useMutation({
    mutationFn: (cliente: ClienteEscolhido) =>
      api.iniciarExtracao({
        token: config.chave,
        clienteId: cliente.id,
        pastaBase: config.pastaBase,
        baixarAnexos: config.baixarAnexos,
        rebaixarTudo: false,
        requisicoesPorMinuto: config.requisicoesPorMinuto,
      }),
    onSuccess: (p) => {
      definirProgresso(p)
      document.getElementById('topo')?.scrollIntoView({ behavior: 'smooth' })
    },
    onError: (e) => toast.error(mensagemDeErro(e)),
  })

  const faltando = !config.chave
    ? 'Informe a chave da API.'
    : !escolhido
      ? 'Busque e escolha um cliente.'
      : !config.pastaBase
        ? 'Escolha a pasta de destino.'
        : null

  return (
    <div className="h-full overflow-y-auto">
      <div id="topo" className="mx-auto flex max-w-6xl flex-col gap-5 p-6">
        {progresso && <ProgressoExtracao progresso={progresso} />}

        <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_380px]">
          <div className="flex min-h-[420px] flex-col lg:h-[calc(100vh-9rem)]">
            <CartaoCliente
              escolhido={escolhido}
              aoEscolher={(c) =>
                navigate({ search: { cliente: c.id, nome: c.nome, tipo: c.tipo }, replace: true })
              }
            />
          </div>

          <div className="flex flex-col gap-5">
            <CartaoChave />
            <CartaoOpcoes />

            <div className="space-y-2">
              <Button
                size="lg"
                className="w-full"
                disabled={!!faltando || rodando || iniciar.isPending}
                onClick={() => escolhido && iniciar.mutate(escolhido)}
              >
                {iniciar.isPending ? <Spinner /> : <DownloadIcon />}
                <span className="truncate">
                  {escolhido ? `Extrair tickets de ${escolhido.nome}` : 'Extrair tickets'}
                </span>
              </Button>
              <p className="text-center text-xs text-muted-foreground">
                {rodando
                  ? 'Aguarde a extração atual terminar.'
                  : (faltando ??
                    `A API aceita ${config.requisicoesPorMinuto} requisições por minuto e cada ticket é uma. Tickets já extraídos e sem mudança são lidos do disco.`)}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

function CartaoOpcoes() {
  const config = useConfiguracoes()
  const idAnexos = useId()
  const idLimite = useId()
  const escolherPasta = useMutation({
    mutationFn: async () => {
      const pasta = await open({
        directory: true,
        defaultPath: config.pastaBase || undefined,
        title: 'Pasta onde salvar as extrações',
      })
      if (typeof pasta === 'string') alterarConfiguracoes({ pastaBase: pasta })
    },
    onError: (e) => toast.error(mensagemDeErro(e)),
  })

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <SlidersHorizontalIcon className="size-4 text-primary" />
          Opções
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-5">
        <Field>
          <FieldLabel>Pasta de destino</FieldLabel>
          <InputGroup>
            <InputGroupAddon>
              <FolderIcon />
            </InputGroupAddon>
            <InputGroupInput readOnly value={config.pastaBase} title={config.pastaBase} className="text-xs" />
            <InputGroupAddon align="inline-end">
              <InputGroupButton size="xs" onClick={() => escolherPasta.mutate()}>
                Trocar
              </InputGroupButton>
            </InputGroupAddon>
          </InputGroup>
          <FieldDescription>Cada cliente ganha uma subpasta com JSON, CSV e as conversas em Markdown.</FieldDescription>
        </Field>

        <Field orientation="horizontal">
          <Checkbox
            id={idAnexos}
            checked={config.baixarAnexos}
            onCheckedChange={(v) => alterarConfiguracoes({ baixarAnexos: v === true })}
          />
          <FieldContent>
            <FieldLabel htmlFor={idAnexos} className="font-normal">
              Baixar também os anexos
            </FieldLabel>
            <FieldDescription>Cada anexo gasta uma requisição a mais; a extração fica bem mais lenta.</FieldDescription>
          </FieldContent>
        </Field>

        <Field>
          <FieldLabel htmlFor={idLimite}>Requisições por minuto</FieldLabel>
          <Input
            id={idLimite}
            type="number"
            min={1}
            max={600}
            className="w-28"
            value={config.requisicoesPorMinuto}
            onChange={(e) => {
              const valor = Number.parseInt(e.target.value, 10)
              if (Number.isFinite(valor) && valor > 0) alterarConfiguracoes({ requisicoesPorMinuto: Math.min(valor, 600) })
            }}
          />
          <FieldDescription>
            O limite documentado é {REQUISICOES_POR_MINUTO_PADRAO}. Só aumente se o Movidesk liberou mais para a sua
            conta: passar do limite bloqueia a chave por alguns minutos.
          </FieldDescription>
        </Field>
      </CardContent>
    </Card>
  )
}
