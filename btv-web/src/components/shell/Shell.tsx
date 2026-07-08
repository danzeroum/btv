import { useRef, useState } from 'react'
import { useAppState } from '../../state/AppContext'
import { useBrand } from '../../state/useBrand'
import { Topbar } from './Topbar'
import { Sidebar } from './Sidebar'
import { GearDrawer } from './GearDrawer'
import { SCREEN_META } from '../../lib/screenMeta'
import { SCREEN_COMPONENTS } from '../../lib/screenComponents'

export function Shell() {
  const rootRef = useRef<HTMLDivElement | null>(null)
  const { screen, accent, squad } = useAppState()
  const [gearOpen, setGearOpen] = useState(false)
  useBrand(rootRef, accent)

  const meta = SCREEN_META[screen]
  const ScreenComponent = SCREEN_COMPONENTS[screen]
  // O kicker da tela Ao vivo usa a cor de identidade da squad ativa (handoff §6).
  const accentColor = screen === 'vivo' && squad ? squad.cor : meta.accent

  return (
    <div id="btv-root" ref={rootRef}>
      <Topbar onToggleGear={() => setGearOpen((v) => !v)} />
      <div className="btv-body">
        <Sidebar />
        <main className="btv-stage">
          <div className="btv-stage-inner">
            <div className="screen-header">
              <div style={{ minWidth: 0 }}>
                <div className="kicker" style={{ color: accentColor }}>
                  {meta.kicker}
                </div>
                <h1 className="screen-title">{meta.title}</h1>
              </div>
              <div className="screen-note">{meta.note}</div>
            </div>
            <ScreenComponent />
          </div>
        </main>
      </div>
      {gearOpen && <GearDrawer onClose={() => setGearOpen(false)} />}
    </div>
  )
}
