import { invoke } from '@tauri-apps/api/core'

/** Pessoa, empresa ou departamento do cadastro do Movidesk (`movidesk::Pessoa`). */
export type Pessoa = {
  id: string
  nome: string
  razaoSocial: string | null
  cpfCnpj: string | null
  email: string | null
  /** 1 = pessoa, 2 = empresa, 4 = departamento. */
  tipoPessoa: number
  /** 1 = agente, 2 = cliente, 3 = agente e cliente. */
  tipoPerfil: number
  ativo: boolean
  organizacoes: { id: string; nome: string }[]
}

export type EstadoExtracao = 'rodando' | 'concluida' | 'cancelada' | 'erro'
export type FaseExtracao = 'cliente' | 'listagem' | 'tickets' | 'anexos' | 'arquivos' | 'fim'

export type Progresso = {
  estado: EstadoExtracao
  fase: FaseExtracao
  mensagem: string
  cliente: Pessoa | null
  membros: number
  ticketsListados: number
  ticketsTotal: number
  ticketsProcessados: number
  ticketsBaixados: number
  ticketsReaproveitados: number
  anexosTotal: number
  anexosProcessados: number
  anexosBaixados: number
  requisicoes: number
  requisicoesPorMinuto: number
  esperaSegundos: number | null
  restanteSegundos: number | null
  iniciadaEm: number
  atualizadaEm: number
  pasta: string | null
  caminho: string | null
  erro: string | null
  avisos: string[]
}

export type Metadados = {
  versao: number
  cliente: Pessoa
  membros: Pessoa[]
  situacao: 'completa' | 'cancelada' | 'erro'
  iniciadaEm: number
  concluidaEm: number
  ticketsListados: number
  ticketsSalvos: number
  anexosIncluidos: boolean
  anexosBaixados: number
  avisos: string[]
  erro: string | null
}

export type ExtracaoResumo = { pasta: string; caminho: string; metadados: Metadados }

export type TicketResumo = {
  id: number
  protocolo: string | null
  assunto: string
  status: string | null
  statusBase: string | null
  categoria: string | null
  urgencia: string | null
  servico: string | null
  clientes: string[]
  responsavel: string | null
  equipe: string | null
  criadoEm: string | null
  ultimaAtualizacao: string | null
  resolvidoEm: string | null
  fechadoEm: string | null
  qtdAcoes: number
  qtdAnexos: number
  origem: number | null
}

export type ExtracaoDetalhe = ExtracaoResumo & { tickets: TicketResumo[] }

/** Pessoa como aparece dentro do ticket (cliente, responsável, autor da ação). */
export type PessoaMovidesk = {
  id: string
  businessName?: string | null
  email?: string | null
  personType?: number
  profileType?: number
  organization?: PessoaMovidesk | null
}

export type AcaoMovidesk = {
  id: number
  /** 1 = interna, 2 = pública. */
  type: number
  origin: number
  description?: string | null
  status?: string | null
  justification?: string | null
  createdDate?: string | null
  createdBy?: PessoaMovidesk | null
  isDeleted?: boolean
  attachments?: { fileName: string; path: string; createdDate?: string | null }[]
  tags?: string[]
}

/** Só os campos que a tela usa; o JSON completo fica salvo em disco. */
export type TicketMovidesk = {
  /** O GET de um ticket devolve o id como texto; a listagem, como número. */
  id: number | string
  protocol?: string | null
  subject?: string | null
  status?: string | null
  baseStatus?: string | null
  category?: string | null
  urgency?: string | null
  serviceFull?: string[] | string | null
  tags?: string[] | null
  clients?: PessoaMovidesk[] | null
  owner?: PessoaMovidesk | null
  ownerTeam?: string | null
  createdDate?: string | null
  resolvedIn?: string | null
  closedIn?: string | null
  lastUpdate?: string | null
  origin?: number | null
  actions?: AcaoMovidesk[] | null
}

export type AnexoLocal = {
  acaoId: number
  hash: string
  nome: string
  caminhoRelativo: string | null
}

export type TicketCompleto = { ticket: TicketMovidesk; anexos: AnexoLocal[] }

export type PedidoExtracao = {
  token: string
  clienteId: string
  pastaBase: string
  baixarAnexos: boolean
  rebaixarTudo: boolean
  requisicoesPorMinuto: number
}

export const api = {
  chavePadrao: () => invoke<string | null>('chave_padrao'),
  pastaPadrao: () => invoke<string>('pasta_padrao'),
  testarChave: (token: string, requisicoesPorMinuto: number) =>
    invoke<void>('testar_chave', { token, requisicoesPorMinuto }),
  buscarClientes: (token: string, termo: string, requisicoesPorMinuto: number) =>
    invoke<Pessoa[]>('buscar_clientes', { token, termo, requisicoesPorMinuto }),
  iniciarExtracao: (pedido: PedidoExtracao) => invoke<Progresso>('iniciar_extracao', { pedido }),
  cancelarExtracao: () => invoke<void>('cancelar_extracao'),
  statusExtracao: () => invoke<Progresso | null>('status_extracao'),
  listarExtracoes: (pastaBase: string) =>
    invoke<ExtracaoResumo[]>('listar_extracoes', { pastaBase }),
  carregarExtracao: (pastaBase: string, pasta: string) =>
    invoke<ExtracaoDetalhe>('carregar_extracao', { pastaBase, pasta }),
  carregarTicket: (pastaBase: string, pasta: string, id: number) =>
    invoke<TicketCompleto>('carregar_ticket', { pastaBase, pasta, id }),
  abrirCaminho: (pastaBase: string, relativo: string) =>
    invoke<void>('abrir_caminho', { pastaBase, relativo }),
  mostrarNaPasta: (pastaBase: string, relativo: string) =>
    invoke<void>('mostrar_na_pasta', { pastaBase, relativo }),
}

/** Os comandos rejeitam com a mensagem em texto; normaliza para exibir. */
export function mensagemDeErro(erro: unknown): string {
  if (typeof erro === 'string') return erro
  if (erro instanceof Error) return erro.message
  return 'Erro inesperado.'
}
