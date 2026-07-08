import { AppProvider } from './state/AppContext'
import { TemplatesProvider } from './state/TemplatesContext'
import { SquadRunProvider } from './state/SquadRunContext'
import { Shell } from './components/shell/Shell'

function App() {
  return (
    <AppProvider>
      {/* Acima do Shell: os 12 modelos são compartilhados por galeria,
          wizard, personas e admin — carregados uma vez do backend real. */}
      <TemplatesProvider>
        {/* Acima da troca de tela: a squad ativa (SSE + gate + cockpit)
            sobrevive à navegação entre telas. */}
        <SquadRunProvider>
          <Shell />
        </SquadRunProvider>
      </TemplatesProvider>
    </AppProvider>
  )
}

export default App
