import type { QueryClient } from '@tanstack/react-query'
import { createRootRouteWithContext, Link, Outlet } from '@tanstack/react-router'
import { FolderOpenIcon, ScanSearchIcon } from 'lucide-react'
import { Progress } from '@/components/ui/progress'
import { Spinner } from '@/components/ui/spinner'
import { useProgresso } from '@/lib/progresso'
import { percentual } from '@/components/progresso-extracao'

export const Route = createRootRouteWithContext<{ queryClient: QueryClient }>()({
  component: Layout,
})

function Layout() {
  return (
    <div className="flex h-full flex-col">
      <div className="h-1 shrink-0 bg-marca" />
      <header className="flex h-14 shrink-0 items-center gap-6 border-b bg-card px-5">
        <Link to="/" className="flex items-center gap-2.5">
          <img src="/icon.svg" alt="" className="size-7" />
          <span className="font-semibold tracking-tight">Movidesk Extrator</span>
        </Link>
        <nav className="flex items-center gap-1 text-sm">
          <ItemMenu to="/" icone={<ScanSearchIcon />} exato>
            Extrair
          </ItemMenu>
          <ItemMenu to="/extracoes" icone={<FolderOpenIcon />}>
            Extrações
          </ItemMenu>
        </nav>
        <IndicadorDeExtracao />
      </header>
      <main className="min-h-0 flex-1">
        <Outlet />
      </main>
    </div>
  )
}

function ItemMenu({
  to,
  icone,
  exato,
  children,
}: {
  to: '/' | '/extracoes'
  icone: React.ReactNode
  exato?: boolean
  children: React.ReactNode
}) {
  return (
    <Link
      to={to}
      activeOptions={{ exact: exato, includeSearch: false }}
      className="flex items-center gap-2 rounded-md px-3 py-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground data-[status=active]:bg-accent data-[status=active]:text-accent-foreground [&_svg]:size-4"
    >
      {icone}
      {children}
    </Link>
  )
}

/** Mostra a extração em andamento em qualquer tela. */
function IndicadorDeExtracao() {
  const progresso = useProgresso()
  if (progresso?.estado !== 'rodando') return null
  return (
    <Link
      to="/"
      className="ml-auto flex items-center gap-3 rounded-full border bg-background px-3 py-1 text-xs hover:bg-muted"
    >
      <Spinner className="size-3.5 text-primary" />
      <span className="max-w-48 truncate font-medium">{progresso.cliente?.nome ?? 'Extraindo'}</span>
      <Progress value={percentual(progresso)} className="h-1.5 w-24" />
    </Link>
  )
}
