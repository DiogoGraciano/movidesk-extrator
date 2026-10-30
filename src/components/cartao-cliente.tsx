import { useQuery } from '@tanstack/react-query'
import { Building2Icon, CheckIcon, SearchIcon, UserIcon, UsersIcon } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty'
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { api, mensagemDeErro, type Pessoa } from '@/lib/api'
import { useConfiguracoes } from '@/lib/configuracoes'
import { documento, tipoPessoa } from '@/lib/formatar'
import { cn } from '@/lib/utils'

export type ClienteEscolhido = { id: string; nome: string; tipo: number }

export function CartaoCliente({
  escolhido,
  aoEscolher,
}: {
  escolhido: ClienteEscolhido | null
  aoEscolher: (cliente: ClienteEscolhido) => void
}) {
  const config = useConfiguracoes()
  const [termo, setTermo] = useState('')
  const [buscado, setBuscado] = useState('')
  // Busca só ao confirmar: cada busca gasta uma das 10 requisições do minuto.
  const busca = useQuery({
    queryKey: ['clientes', buscado],
    queryFn: () => api.buscarClientes(config.chave, buscado, config.requisicoesPorMinuto),
    enabled: !!config.chave && buscado.length >= 2,
    staleTime: 5 * 60_000,
  })
  const podeBuscar = !!config.chave && termo.trim().length >= 2

  return (
    <Card className="flex min-h-0 flex-1 flex-col">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <UsersIcon className="size-4 text-primary" />
          Cliente
        </CardTitle>
        <CardDescription>
          Busque pelo nome, razão social, CNPJ/CPF, e-mail ou código. Escolhendo uma empresa, entram os
          tickets dela e de todas as pessoas vinculadas.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-4">
        <form
          onSubmit={(e) => {
            e.preventDefault()
            if (podeBuscar) setBuscado(termo.trim())
          }}
        >
          <InputGroup>
            <InputGroupAddon>
              <SearchIcon />
            </InputGroupAddon>
            <InputGroupInput
              autoFocus
              placeholder={config.chave ? 'Nome, razão social, CNPJ/CPF, e-mail ou código' : 'Informe a chave da API primeiro'}
              value={termo}
              disabled={!config.chave}
              onChange={(e) => setTermo(e.target.value)}
            />
            <InputGroupAddon align="inline-end">
              <InputGroupButton type="submit" variant="default" size="sm" disabled={!podeBuscar}>
                {busca.isFetching ? <Spinner /> : null}
                Buscar
              </InputGroupButton>
            </InputGroupAddon>
          </InputGroup>
        </form>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {busca.isFetching && !busca.data ? (
            <div className="space-y-2">
              {[0, 1, 2].map((i) => (
                <Skeleton key={i} className="h-16 w-full" />
              ))}
            </div>
          ) : busca.isError ? (
            <Alert variant="destructive">
              <AlertDescription>{mensagemDeErro(busca.error)}</AlertDescription>
            </Alert>
          ) : busca.data?.length === 0 ? (
            <Empty className="border">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <SearchIcon />
                </EmptyMedia>
                <EmptyTitle>Nenhum cliente encontrado</EmptyTitle>
                <EmptyDescription>Nada no cadastro do Movidesk bate com “{buscado}”.</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ) : busca.data ? (
            <ul className="space-y-2" aria-label="Clientes encontrados">
              {busca.data.map((pessoa) => (
                <ItemCliente
                  key={pessoa.id}
                  pessoa={pessoa}
                  escolhido={escolhido?.id === pessoa.id}
                  aoEscolher={aoEscolher}
                  escolhidoId={escolhido?.id}
                />
              ))}
            </ul>
          ) : escolhido ? (
            <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
              Cliente escolhido: <strong className="text-foreground">{escolhido.nome}</strong> (código{' '}
              {escolhido.id}). Busque para trocar.
            </p>
          ) : (
            <Empty className="h-full">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Building2Icon />
                </EmptyMedia>
                <EmptyTitle>Qual cliente extrair?</EmptyTitle>
                <EmptyDescription>
                  Cada busca usa uma requisição da API, por isso ela só roda ao apertar Enter ou Buscar.
                </EmptyDescription>
              </EmptyHeader>
            </Empty>
          )}
        </div>
      </CardContent>
    </Card>
  )
}

function ItemCliente({
  pessoa,
  escolhido,
  escolhidoId,
  aoEscolher,
}: {
  pessoa: Pessoa
  escolhido: boolean
  escolhidoId: string | undefined
  aoEscolher: (cliente: ClienteEscolhido) => void
}) {
  const Icone = pessoa.tipoPessoa === 1 ? UserIcon : Building2Icon
  const detalhes = [
    pessoa.razaoSocial !== pessoa.nome ? pessoa.razaoSocial : null,
    documento(pessoa.cpfCnpj),
    pessoa.email,
  ].filter(Boolean)

  return (
    <li>
      <div
        className={cn(
          'flex items-start gap-3 rounded-lg border p-3 transition-colors',
          escolhido ? 'border-primary bg-accent/60' : 'hover:bg-muted/60',
        )}
      >
        <button
          type="button"
          aria-pressed={escolhido}
          className="flex min-w-0 flex-1 items-start gap-3 text-left outline-none"
          onClick={() => aoEscolher({ id: pessoa.id, nome: pessoa.nome, tipo: pessoa.tipoPessoa })}
        >
          <span
            className={cn(
              'mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground',
              escolhido && 'bg-primary text-primary-foreground',
            )}
          >
            {escolhido ? <CheckIcon className="size-4" /> : <Icone className="size-4" />}
          </span>
          <span className="min-w-0 flex-1 space-y-1">
            <span className="flex flex-wrap items-center gap-1.5">
              <span className="font-medium">{pessoa.nome}</span>
              <Badge variant="secondary">{tipoPessoa(pessoa.tipoPessoa)}</Badge>
              {pessoa.tipoPerfil === 1 && <Badge variant="outline">Agente</Badge>}
              {!pessoa.ativo && <Badge variant="outline">Inativo</Badge>}
            </span>
            {detalhes.length > 0 && (
              <span className="block truncate text-xs text-muted-foreground">{detalhes.join(' · ')}</span>
            )}
          </span>
          <span className="shrink-0 font-mono text-xs text-muted-foreground">{pessoa.id}</span>
        </button>
      </div>
      {pessoa.tipoPessoa === 1 && pessoa.organizacoes.length > 0 && (
        <div className="mt-1 flex flex-wrap items-center gap-1 pl-14 text-xs text-muted-foreground">
          Vinculada a
          {pessoa.organizacoes.map((org) => (
            <Button
              key={org.id}
              variant="link"
              size="sm"
              className={cn('h-auto p-0 text-xs', escolhidoId === org.id && 'font-semibold')}
              onClick={() => aoEscolher({ id: org.id, nome: org.nome || org.id, tipo: 2 })}
            >
              {org.nome || org.id}
            </Button>
          ))}
          <span>(escolher a empresa inteira)</span>
        </div>
      )}
    </li>
  )
}
