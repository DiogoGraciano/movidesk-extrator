import { useSyncExternalStore } from 'react'
import { api } from '@/lib/api'

export type Configuracoes = {
  chave: string
  lembrarChave: boolean
  pastaBase: string
  requisicoesPorMinuto: number
  baixarAnexos: boolean
}

const CHAVE_STORAGE = 'movidesk-extrator:configuracoes'

/** Limite documentado da API do Movidesk. */
export const REQUISICOES_POR_MINUTO_PADRAO = 10

let atual: Configuracoes = {
  chave: '',
  lembrarChave: true,
  pastaBase: '',
  requisicoesPorMinuto: REQUISICOES_POR_MINUTO_PADRAO,
  baixarAnexos: false,
}
const ouvintes = new Set<() => void>()

function persistir() {
  // Sem "lembrar", a chave fica só na memória e some ao fechar o app.
  const salvar = atual.lembrarChave ? atual : { ...atual, chave: '' }
  localStorage.setItem(CHAVE_STORAGE, JSON.stringify(salvar))
}

/** Lê o que foi salvo e completa a chave e a pasta com os padrões do Rust. */
export async function carregarConfiguracoes() {
  try {
    const salvo = JSON.parse(localStorage.getItem(CHAVE_STORAGE) ?? '{}') as Partial<Configuracoes>
    atual = { ...atual, ...salvo }
  } catch {
    localStorage.removeItem(CHAVE_STORAGE)
  }
  const [chave, pasta] = await Promise.all([
    atual.chave ? null : api.chavePadrao().catch(() => null),
    atual.pastaBase ? null : api.pastaPadrao().catch(() => ''),
  ])
  if (chave) atual = { ...atual, chave }
  if (pasta) atual = { ...atual, pastaBase: pasta }
}

export function lerConfiguracoes() {
  return atual
}

export function alterarConfiguracoes(parcial: Partial<Configuracoes>) {
  atual = { ...atual, ...parcial }
  persistir()
  for (const ouvinte of ouvintes) ouvinte()
}

function inscrever(ouvinte: () => void) {
  ouvintes.add(ouvinte)
  return () => ouvintes.delete(ouvinte)
}

export function useConfiguracoes() {
  return useSyncExternalStore(inscrever, lerConfiguracoes)
}
