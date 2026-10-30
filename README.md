<img src="assets/icon.svg" width="64" height="64" alt="">

# Movidesk Extrator

[![Build](https://github.com/DiogoGraciano/movidesk-extrator/actions/workflows/build.yml/badge.svg)](https://github.com/DiogoGraciano/movidesk-extrator/actions/workflows/build.yml)

App desktop (Tauri 2 + React) que extrai **todos os tickets de um cliente** do Movidesk, com o
**histórico completo de conversa** de cada um, e grava tudo em disco: JSON completo, CSV e uma
conversa legível em Markdown por ticket. As extrações ficam navegáveis dentro do próprio app.

## Download

Os instaladores ficam na [última release](https://github.com/DiogoGraciano/movidesk-extrator/releases/latest):
`.exe` para Windows, `.deb` ou `.AppImage` para Linux e `.dmg` para macOS.

- **Linux:** os pacotes são compilados no Ubuntu 24.04 e servem para distros com glibc 2.39 ou
  mais nova (Ubuntu 24.04, Debian 13, Fedora 40, Linux Mint 22 ou mais recentes).
- **macOS:** o `.dmg` é universal (Apple Silicon e Intel). O app tem só assinatura ad-hoc, sem
  notarização da Apple, então o macOS bloqueia a primeira abertura. Para liberar, abra
  **Ajustes do Sistema › Privacidade e Segurança**, role até o fim, clique em
  **Abrir Mesmo Assim** e confirme em **Abrir**. Depois disso ele abre normalmente.

## Como usar

1. Informe a **chave da API** (no Movidesk: Configurações › Conta › Parâmetros › Ambiente ›
   Gerar nova chave). Em `bun run app:dev` ela vem preenchida do `MOVIDESK_API_KEY` do `.env`.
2. **Busque o cliente** por nome, razão social, CNPJ/CPF, e-mail ou código e escolha um.
   - **Empresa ou departamento:** entram os tickets da organização e de todas as pessoas
     vinculadas a ela.
   - **Pessoa:** entram só os tickets em que ela é cliente. Se ela estiver vinculada a uma
     empresa, o link abaixo do nome troca para a empresa inteira.
3. Clique em **Extrair tickets**. A extração roda em segundo plano, com o progresso e uma
   estimativa de tempo, e pode ser cancelada.
4. Em **Extrações** ficam os clientes já extraídos: a lista de tickets com filtro, a conversa de
   cada ticket ação por ação, com os anexos, e o botão para abrir a pasta.

### O que é gravado

```text
<pasta de destino>/<Nome do cliente> (<código>)/
  extracao.json          cliente, pessoas vinculadas, situação, contagens e avisos
  tickets.json           todos os tickets completos, como a API devolve
  tickets.csv            um ticket por linha, separado por ";" (abre direto no Excel)
  tickets/<id>.json      cada ticket completo (serve também de cache)
  conversas/<id> - <assunto>.md   a conversa legível
  anexos/<ticket>/<ação>-<arquivo> com a opção "Baixar também os anexos"
```

A pasta de destino padrão é `~/Documentos/Movidesk Extrator` e pode ser trocada em **Opções**.

## Limite da API e tempo de extração

A API do Movidesk aceita **10 requisições por minuto**, e cada ticket custa uma requisição.
Por isso 100 tickets levam cerca de 10 minutos, e cada anexo é uma requisição a mais. O app
respeita o limite sozinho com uma janela deslizante de 1 minuto. Quando recebe 429, espera o
`retry-after`. Falhas de rede e 5xx são repetidas com espera crescente, porque 3 falhas
seguidas bloqueiam a chave.

Para não repetir trabalho, um ticket só é baixado de novo se a `lastUpdate` dele mudou. Por isso
**"Extrair de novo"** num cliente já extraído gasta só a listagem e os tickets novos ou alterados.
Uma extração cancelada ou interrompida continua de onde parou.

Se o Movidesk liberou um limite maior para a conta, ajuste **Requisições por minuto** em Opções.

## Como a extração funciona

Os comportamentos abaixo foram conferidos contra a API real:

| Etapa | Chamada | Observação |
|---|---|---|
| Busca de cliente | `GET /persons` com `contains(businessName, …)` / `contains(corporateName, …)` | Não diferencia maiúsculas. |
| Pessoas da empresa | `GET /persons` com `relationships/any(r: r/id eq '<empresa>')` | |
| Listagem | `GET /tickets` e `GET /tickets/past` com `clients/any(c: c/id eq … or …)` | Filtrar por `c/organization/id` em `/tickets/past` leva ~30s e estoura o timeout do servidor. Filtrar pelo id da empresa e das pessoas leva 3 a 5s. |
| Listagem extra (empresa) | `GET /tickets` com `clients/any(c: c/organization/id eq …)` | Pega tickets recentes de quem já saiu da empresa. Em `/tickets` leva ~4s. |
| Conversa | `GET /tickets?id=…` (ou `/tickets/past?id=…`) | Traz todas as ações, anexos, apontamentos e histórico de status. |
| Anexo | `GET /storage/download?id=<hash>` | Redireciona para o storage e para o S3; só a primeira chamada conta no limite. |

`/tickets` cobre os tickets atualizados nos últimos 90 dias e `/tickets/past` os mais antigos.
As duas rotas se sobrepõem, então os resultados são juntados por id.

**Limitação conhecida:** tickets com mais de 90 dias de alguém que **já não está vinculado** à
empresa não entram na extração da empresa. A única consulta que os acharia (`organization/id`
em `/tickets/past`) estoura o timeout da API. Os tickets dessa pessoa podem ser extraídos
buscando-a diretamente.

## Desenvolvimento

Requisitos: [Bun](https://bun.sh), Rust estável e as
[dependências do Tauri](https://tauri.app/start/prerequisites/) (no Linux, `webkit2gtk-4.1`).

```sh
bun install
bun run app:dev          # abre o app com recarga automática
bun run typecheck        # TypeScript
bun run app:build        # instaladores em src-tauri/target/release/bundle
```

O workflow `Build` publica uma release a cada push na `main`, depois que os builds de Windows,
Linux e macOS passam. Pushes que só mudam arquivos `.md` não geram release. Pull requests só compilam e
deixam os instaladores como artefatos da execução em
[Actions](https://github.com/DiogoGraciano/movidesk-extrator/actions/workflows/build.yml).

A versão sobe sozinha no último número (0.1.0, 0.1.1, 0.1.2...), a partir da última tag `v*`.
Para ir para 0.2.0 ou 1.0.0, troque a versão em `src-tauri/tauri.conf.json` (e, para manter
igual, em `package.json` e `src-tauri/Cargo.toml`): a próxima release sai com ela.

A chave da API nunca vai para o repositório nem para o CI. Copie `.env.example` para `.env`
(ignorado pelo git) e preencha a sua.

Testes do Rust:

```sh
cd src-tauri
cargo test                                   # unitários
# Contra a API real (usa o MOVIDESK_API_KEY do .env e grava em /tmp/movidesk-extrator-teste):
MOVIDESK_TESTE_BUSCA=<nome> cargo test busca_e_lista_na_api_real -- --ignored --nocapture
MOVIDESK_TESTE_CLIENTE=<código> cargo test extrai_cliente_na_api_real -- --ignored --nocapture
```

### Estrutura

- `src-tauri/src/movidesk.rs`: cliente HTTP da API, com limitador, novas tentativas, busca,
  listagem, ticket e anexo.
- `src-tauri/src/extracao.rs`: as fases da extração, o progresso emitido como evento
  (`extracao://progresso`) e o cancelamento.
- `src-tauri/src/saida.rs`: a pasta do cliente, o cache, a geração de JSON, CSV e Markdown e a
  leitura das extrações salvas.
- `src/routes`: telas (TanStack Router com rotas por arquivo, histórico por hash).
- `src/lib`: comandos Tauri tipados, configurações (localStorage), progresso e formatação.
- `src/components/ui`: componentes shadcn/ui.

A chave da API fica no `localStorage` do app quando "Lembrar a chave" está marcado. Sem essa
opção, ela fica só na memória e some ao fechar o app. Todas as chamadas à API saem do Rust e
nunca do webview.
