import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createHashHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import { carregarConfiguracoes } from '@/lib/configuracoes'
import { escutarProgresso } from '@/lib/progresso'
import { routeTree } from './routeTree.gen'
import './styles.css'

const queryClient = new QueryClient({
  defaultOptions: {
    // Os dados vêm do disco local ou da API com limite de 10 req/min: nada de
    // refetch automático ao focar a janela nem nova tentativa silenciosa.
    queries: { staleTime: 30_000, refetchOnWindowFocus: false, retry: false },
  },
})

// Hash history: o webview do Tauri não tem fallback de rota para o index.html.
const router = createRouter({
  routeTree,
  history: createHashHistory(),
  context: { queryClient },
  defaultPreload: 'intent',
  defaultPreloadStaleTime: 0,
  scrollRestoration: true,
})

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

function seguirTemaDoSistema() {
  const escuro = window.matchMedia('(prefers-color-scheme: dark)')
  const aplicar = () => document.documentElement.classList.toggle('dark', escuro.matches)
  aplicar()
  escuro.addEventListener('change', aplicar)
}

async function iniciar() {
  seguirTemaDoSistema()
  await Promise.all([carregarConfiguracoes(), escutarProgresso(queryClient)])
  createRoot(document.getElementById('root') as HTMLElement).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <TooltipProvider delayDuration={300}>
          <RouterProvider router={router} />
          <Toaster richColors position="bottom-right" />
        </TooltipProvider>
      </QueryClientProvider>
    </StrictMode>,
  )
}

void iniciar()
