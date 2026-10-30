import { useMutation } from '@tanstack/react-query'
import { CheckIcon, EyeIcon, EyeOffIcon, KeyRoundIcon } from 'lucide-react'
import { useId, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Field, FieldDescription, FieldError, FieldLabel } from '@/components/ui/field'
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { api, mensagemDeErro } from '@/lib/api'
import { alterarConfiguracoes, useConfiguracoes } from '@/lib/configuracoes'

export function CartaoChave() {
  const config = useConfiguracoes()
  const [visivel, setVisivel] = useState(false)
  const idCampo = useId()
  const idLembrar = useId()
  const testar = useMutation({
    mutationFn: () => api.testarChave(config.chave, config.requisicoesPorMinuto),
  })

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <KeyRoundIcon className="size-4 text-primary" />
          Chave da API
        </CardTitle>
        <CardDescription>
          No Movidesk: Configurações › Conta › Parâmetros › Ambiente › Gerar nova chave.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <Field data-invalid={testar.isError || undefined}>
          <FieldLabel htmlFor={idCampo}>Token</FieldLabel>
          <InputGroup>
            <InputGroupInput
              id={idCampo}
              type={visivel ? 'text' : 'password'}
              autoComplete="off"
              spellCheck={false}
              placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
              className="font-mono text-xs"
              value={config.chave}
              aria-invalid={testar.isError || undefined}
              onChange={(e) => {
                testar.reset()
                alterarConfiguracoes({ chave: e.target.value.trim() })
              }}
            />
            <InputGroupAddon align="inline-end">
              <InputGroupButton
                size="icon-xs"
                aria-label={visivel ? 'Esconder a chave' : 'Mostrar a chave'}
                onClick={() => setVisivel((v) => !v)}
              >
                {visivel ? <EyeOffIcon /> : <EyeIcon />}
              </InputGroupButton>
            </InputGroupAddon>
          </InputGroup>
          {testar.isError ? (
            <FieldError>{mensagemDeErro(testar.error)}</FieldError>
          ) : testar.isSuccess ? (
            <FieldDescription className="flex items-center gap-1.5 text-success">
              <CheckIcon className="size-3.5" /> Chave aceita pelo Movidesk.
            </FieldDescription>
          ) : null}
        </Field>
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Checkbox
              id={idLembrar}
              checked={config.lembrarChave}
              onCheckedChange={(v) => alterarConfiguracoes({ lembrarChave: v === true })}
            />
            <Label htmlFor={idLembrar} className="font-normal">
              Lembrar a chave
            </Label>
          </div>
          <Button
            variant="outline"
            size="sm"
            disabled={!config.chave || testar.isPending}
            onClick={() => testar.mutate()}
          >
            {testar.isPending && <Spinner />}
            Testar chave
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
