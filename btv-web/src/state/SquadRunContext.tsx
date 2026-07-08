import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { useAppDispatch } from './AppContext'
import { ativarSquad, aprovarGate, pedirAjuste, type AtivarSquadPayload, type BtvRun } from '../api/btv'
import {
  connectSquadEvents,
  emergencyStopSquad,
  postSquadMessage,
  type SquadChatMessage,
  type SquadEventEnvelope,
} from '../api/squad'
import {
  esteiraFromEvents,
  feedFromEvents,
  makeEtapas,
  type AcaoLocal,
  type Etapa,
  type EsteiraView,
  type FeedItem,
} from '../lib/esteira'
import type { SquadTemplate } from '../api/templates'

interface RunState {
  template: SquadTemplate
  nome: string
  etapas: Etapa[]
  taskId: string
  /** Execução de teste do Designer (U5) — rotulada na tela. */
  teste: boolean
  events: SquadEventEnvelope[]
  acoes: AcaoLocal[]
  streamEnded: boolean
}

export interface SquadRunApi {
  run: RunState | null
  view: EsteiraView | null
  feed: FeedItem[]
  chat: Array<SquadChatMessage & { ts: string }>
  ativar: (template: SquadTemplate, payload: Omit<AtivarSquadPayload, 'template_id'>) => Promise<void>
  /** Reconecta a um run persistido (U6 → Ao vivo): o SSE reemite o snapshot
   *  completo e a esteira é recomputada dos eventos reais. */
  abrirRun: (run: BtvRun, template: SquadTemplate) => void
  aprovar: () => Promise<void>
  ajustar: (instrucao: string) => Promise<void>
  enviarChat: (texto: string) => Promise<void>
  encerrar: () => Promise<void>
}

const SquadRunContext = createContext<SquadRunApi | null>(null)

function hhmm(ts: string): string {
  const m = ts.match(/T(\d{2}):(\d{2})/)
  return m ? `${m[1]}:${m[2]}` : ts.slice(0, 5)
}

export function SquadRunProvider({ children }: { children: ReactNode }) {
  const [run, setRun] = useState<RunState | null>(null)
  const dispatch = useAppDispatch()
  const disconnectRef = useRef<(() => void) | null>(null)

  const conectar = useCallback((taskId: string) => {
    disconnectRef.current?.()
    const close = connectSquadEvents(taskId, {
      onEvent: (event) => {
        setRun((r) => (r && r.taskId === taskId ? { ...r, events: [...r.events, event] } : r))
      },
      onConnectionError: () => {
        // Tarefa de squad é finita: o stream fecha quando ela termina (ver
        // SquadHub::finish_task) — fim de stream e queda de conexão são
        // indistinguíveis pela API do EventSource; tratamos igual.
        close()
        setRun((r) => (r && r.taskId === taskId ? { ...r, streamEnded: true } : r))
      },
    })
    disconnectRef.current = close
  }, [])

  useEffect(() => () => disconnectRef.current?.(), [])

  const ativar = useCallback<SquadRunApi['ativar']>(
    async (template, payload) => {
      const resp = await ativarSquad({ ...payload, template_id: template.id })
      const etapas = makeEtapas(template, payload.papeis_off)
      setRun({
        template,
        nome: payload.nome ?? template.nome,
        etapas,
        taskId: resp.task_id,
        teste: false,
        events: [],
        acoes: [],
        streamEnded: false,
      })
      conectar(resp.task_id)
      dispatch({ type: 'CLOSE_WIZARD' })
      dispatch({ type: 'SET_SCREEN', screen: 'vivo' })
    },
    [conectar, dispatch],
  )

  const abrirRun = useCallback<SquadRunApi['abrirRun']>(
    (btvRun, template) => {
      const papeisAtivos: string[] = JSON.parse(btvRun.papeis_json || '[]') as string[]
      const papeisOff = template.papeis
        .map((p, i) => (papeisAtivos.includes(p) ? -1 : i))
        .filter((i) => i >= 0)
      setRun({
        template,
        nome: btvRun.nome,
        etapas: makeEtapas(template, papeisOff),
        taskId: btvRun.task_id,
        teste: false,
        events: [],
        acoes: [],
        streamEnded: false,
      })
      conectar(btvRun.task_id)
      dispatch({ type: 'SET_SCREEN', screen: 'vivo' })
    },
    [conectar, dispatch],
  )

  const view = useMemo(
    () => (run ? esteiraFromEvents(run.etapas, run.events, run.acoes, run.streamEnded) : null),
    [run],
  )

  const feed = useMemo(() => (run ? feedFromEvents(run.events) : []), [run])

  const chat = useMemo(() => {
    if (!run) return []
    return run.events.flatMap((e) =>
      e.payload && 'Chat' in e.payload ? [{ ...e.payload.Chat, ts: hhmm(e.ts) }] : [],
    )
  }, [run])

  // Chip da topbar + seção SQUAD ATIVA da sidebar refletem o estado real.
  useEffect(() => {
    if (!run || !view) {
      dispatch({ type: 'SET_SQUAD', squad: null })
      return
    }
    dispatch({
      type: 'SET_SQUAD',
      squad: {
        nome: run.nome,
        cor: run.template.cor,
        status: view.done ? 'concluída' : view.gateOpen ? 'aguardando você' : 'em produção',
        gateAberto: view.gateOpen,
      },
    })
  }, [run, view, dispatch])

  const aprovar = useCallback(async () => {
    if (!run || !view) return
    const etapa = run.etapas[Math.min(view.idx, run.etapas.length - 1)]?.nome ?? ''
    await aprovarGate(run.taskId, etapa)
    setRun((r) =>
      r ? { ...r, acoes: [...r.acoes, { kind: 'gate_aprovado', afterEventIndex: r.events.length }] } : r,
    )
  }, [run, view])

  const ajustar = useCallback(
    async (instrucao: string) => {
      if (!run || !view) return
      const etapa = run.etapas[Math.min(view.idx, run.etapas.length - 1)]?.nome ?? ''
      await pedirAjuste(run.taskId, instrucao, etapa)
      setRun((r) =>
        r ? { ...r, acoes: [...r.acoes, { kind: 'ajuste', afterEventIndex: r.events.length }] } : r,
      )
    },
    [run, view],
  )

  const enviarChat = useCallback(
    async (texto: string) => {
      if (!run) return
      await postSquadMessage(run.taskId, texto)
    },
    [run],
  )

  const encerrar = useCallback(async () => {
    if (!run) return
    await emergencyStopSquad(run.taskId, 'encerrado pelo usuário na tela Ao vivo')
    disconnectRef.current?.()
    setRun(null)
    dispatch({ type: 'SET_SQUAD', squad: null })
  }, [run, dispatch])

  const value = useMemo<SquadRunApi>(
    () => ({ run, view, feed, chat, ativar, abrirRun, aprovar, ajustar, enviarChat, encerrar }),
    [run, view, feed, chat, ativar, abrirRun, aprovar, ajustar, enviarChat, encerrar],
  )

  return <SquadRunContext.Provider value={value}>{children}</SquadRunContext.Provider>
}

export function useSquadRun(): SquadRunApi {
  const ctx = useContext(SquadRunContext)
  if (!ctx) throw new Error('useSquadRun deve ser usado dentro de <SquadRunProvider>')
  return ctx
}
