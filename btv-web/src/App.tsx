import { AppProvider } from './state/AppContext'
import { TemplatesProvider } from './state/TemplatesContext'
import { SquadRunProvider } from './state/SquadRunContext'
import { ToastProvider } from './components/primitives'
import { Shell } from './components/shell/Shell'

function App() {
  return (
    <AppProvider>
      {/* No topo de tudo: o Toast é feedback global. Fica acima do
          SquadRunProvider para que a squad ao vivo possa avisar (gate
          encerrado etc.) sem `window.alert`. */}
      <ToastProvider>
        {/* Acima do Shell: os 12 modelos são compartilhados por galeria,
            wizard, personas e admin — carregados uma vez do backend real. */}
        <TemplatesProvider>
          {/* Acima da troca de tela: a squad ativa (SSE + gate + cockpit)
              sobrevive à navegação entre telas. */}
          <SquadRunProvider>
            <Shell />
          </SquadRunProvider>
        </TemplatesProvider>
      </ToastProvider>
    </AppProvider>
  )
}

export default App
