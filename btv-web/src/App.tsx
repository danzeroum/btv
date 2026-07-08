import { AppProvider } from './state/AppContext'
import { Shell } from './components/shell/Shell'

function App() {
  return (
    <AppProvider>
      <Shell />
    </AppProvider>
  )
}

export default App
