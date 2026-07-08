import { AppProvider } from './state/AppContext'
import { TemplatesProvider } from './state/TemplatesContext'
import { Shell } from './components/shell/Shell'

function App() {
  return (
    <AppProvider>
      {/* Acima do Shell: os 12 modelos são compartilhados por galeria,
          wizard, personas e admin — carregados uma vez do backend real. */}
      <TemplatesProvider>
        <Shell />
      </TemplatesProvider>
    </AppProvider>
  )
}

export default App
