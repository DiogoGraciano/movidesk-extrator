import { queryOptions } from '@tanstack/react-query'
import { api } from '@/lib/api'

export const extracoesQuery = (pastaBase: string) =>
  queryOptions({
    queryKey: ['extracoes', pastaBase],
    queryFn: () => api.listarExtracoes(pastaBase),
    enabled: !!pastaBase,
  })

export const extracaoQuery = (pastaBase: string, pasta: string) =>
  queryOptions({
    queryKey: ['extracao', pastaBase, pasta],
    queryFn: () => api.carregarExtracao(pastaBase, pasta),
  })

export const ticketQuery = (pastaBase: string, pasta: string, id: number) =>
  queryOptions({
    queryKey: ['ticket', pastaBase, pasta, id],
    queryFn: () => api.carregarTicket(pastaBase, pasta, id),
  })
