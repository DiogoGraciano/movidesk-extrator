const formatoData = new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short', timeStyle: 'short' })
const formatoDia = new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short' })

/** A API devolve datas em UTC sem fuso ("2026-07-27T17:48:40.9947584"). */
function paraDate(valor: string | number) {
  if (typeof valor === 'number') return new Date(valor)
  const temFuso = /(Z|[+-]\d\d:?\d\d)$/.test(valor)
  // Date só aceita até milissegundos na fração.
  const iso = valor.replace(/(\.\d{3})\d+/, '$1')
  return new Date(temFuso ? iso : `${iso}Z`)
}

export function dataHora(valor: string | number | null | undefined) {
  if (valor == null || valor === '') return '—'
  const d = paraDate(valor)
  return Number.isNaN(d.getTime()) ? String(valor) : formatoData.format(d)
}

export function dia(valor: string | number | null | undefined) {
  if (valor == null || valor === '') return '—'
  const d = paraDate(valor)
  return Number.isNaN(d.getTime()) ? String(valor) : formatoDia.format(d)
}

export function duracao(segundos: number) {
  if (segundos < 60) return `${Math.max(1, Math.round(segundos))}s`
  const minutos = Math.round(segundos / 60)
  if (minutos < 60) return `${minutos} min`
  const horas = Math.floor(minutos / 60)
  const resto = minutos % 60
  return resto ? `${horas}h ${resto}min` : `${horas}h`
}

export function documento(valor: string | null | undefined) {
  if (!valor) return null
  const d = valor.replace(/\D/g, '')
  if (d.length === 14) return d.replace(/^(\d{2})(\d{3})(\d{3})(\d{4})(\d{2})$/, '$1.$2.$3/$4-$5')
  if (d.length === 11) return d.replace(/^(\d{3})(\d{3})(\d{3})(\d{2})$/, '$1.$2.$3-$4')
  return valor
}

export function tipoPessoa(tipo: number) {
  if (tipo === 2) return 'Empresa'
  if (tipo === 4) return 'Departamento'
  return 'Pessoa'
}

export const nomesStatusBase: Record<string, string> = {
  New: 'Novo',
  InAttendance: 'Em atendimento',
  Stopped: 'Parado',
  Canceled: 'Cancelado',
  Resolved: 'Resolvido',
  Closed: 'Fechado',
}

/** Mesma tabela do `saida::nome_da_origem` no Rust. */
const origens: Record<number, string> = {
  0: 'Primeira ação',
  1: 'Via web pelo cliente',
  2: 'Via web pelo agente',
  3: 'Recebido via e-mail',
  4: 'Gatilho do sistema',
  5: 'Chat (online)',
  6: 'Chat (offline)',
  7: 'E-mail enviado pelo sistema',
  8: 'Formulário de contato',
  9: 'Via API',
  10: 'Agendamento automático',
  11: 'Jira',
  12: 'Redmine',
  13: 'Telefone: recebida',
  14: 'Telefone: realizada',
  15: 'Telefone: perdida',
  16: 'Telefone: abandono na fila',
  17: 'Acesso remoto',
  18: 'WhatsApp',
  19: 'Integração Movidesk',
  20: 'Zenvia Chat',
  21: 'Telefone: não atendida',
  22: 'Facebook Messenger',
  23: 'WhatsApp Business Movidesk',
  24: 'Altu',
  25: 'WhatsApp Ativo',
}

export function origem(codigo: number | null | undefined) {
  if (codigo == null) return null
  return origens[codigo] ?? 'Outra origem'
}

export function plural(qtd: number, singular: string, pluralForma = `${singular}s`) {
  return `${qtd.toLocaleString('pt-BR')} ${qtd === 1 ? singular : pluralForma}`
}
