import type { QueryClient } from '@tanstack/react-query'
import { listen } from '@tauri-apps/api/event'
import { useEffect, useState, useSyncExternalStore } from 'react'
import { toast } from 'sonner'
import { api, type Progresso } from '@/lib/api'

const EVENTO = 'extracao://progresso'

let atual: Progresso | null = null
const ouvintes = new Set<() => void>()

function publicar(progresso: Progresso | null) {
  atual = progresso
  for (const ouvinte of ouvintes) ouvinte()
}

/**
 * A extração roda no Rust e continua mesmo se a tela mudar de rota. Aqui o
 * front pega o último estado (caso o webview tenha recarregado) e passa a
 * ouvir os eventos; ao terminar, invalida as consultas das extrações salvas.
 */
export async function escutarProgresso(queryClient: QueryClient) {
  publicar(await api.statusExtracao().catch(() => null))
  await listen<Progresso>(EVENTO, ({ payload }) => {
    const anterior = atual
    publicar(payload)
    if (anterior?.estado === 'rodando' && payload.estado !== 'rodando') {
      void queryClient.invalidateQueries({ queryKey: ['extracoes'] })
      void queryClient.invalidateQueries({ queryKey: ['extracao'] })
      void queryClient.invalidateQueries({ queryKey: ['ticket'] })
      const nome = payload.cliente?.nome ?? 'cliente'
      if (payload.estado === 'concluida') {
        toast.success(`Extração de ${nome} concluída`, {
          description: `${payload.ticketsTotal} tickets salvos.`,
        })
      } else if (payload.estado === 'erro') {
        toast.error(`A extração de ${nome} parou`, { description: payload.erro ?? undefined })
      }
    }
  })
}

export function definirProgresso(progresso: Progresso) {
  publicar(progresso)
}

function inscrever(ouvinte: () => void) {
  ouvintes.add(ouvinte)
  return () => ouvintes.delete(ouvinte)
}

export function useProgresso() {
  return useSyncExternalStore(inscrever, () => atual)
}

/**
 * Segundos que faltam de uma contagem emitida pelo Rust em `desde` (epoch ms),
 * atualizando a cada segundo sem precisar de novos eventos.
 */
export function useContagem(segundos: number | null | undefined, desde: number | undefined) {
  const [agora, setAgora] = useState(() => Date.now())
  const ativo = segundos != null && desde != null
  useEffect(() => {
    if (!ativo) return
    const id = window.setInterval(() => setAgora(Date.now()), 1000)
    return () => window.clearInterval(id)
  }, [ativo])
  if (!ativo) return null
  return Math.max(0, Math.round(segundos - (agora - desde) / 1000))
}
