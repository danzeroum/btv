import { useMemo, useRef, useState, type CSSProperties } from 'react'
import { AuditLedger, type BpmnDiagram } from '@bpmn-react/core'
import { BpmnEditor } from '@bpmn-react/react'
import { VersionRegistry } from '@bpmn-react/registry'
import '@bpmn-react/react/styles.css'
import { btvDesignerPlugin, BLOCO_META } from '../../../designer/btvPlugin'
import { baseDoModelo, baseInicial, baseVazia } from '../../../designer/bases'
import { descricaoDoFluxo, etapasDoFluxo, ordemDoFluxo } from '../../../designer/flow'
import { useTemplates } from '../../../state/TemplatesContext'
import { useSquadRun } from '../../../state/SquadRunContext'
import { fetchJson } from '../../../api/client'

const ghost: CSSProperties = {
  background: 'none',
  border: '1px solid var(--line2)',
  borderRadius: 9,
  padding: '8px 14px',
  fontFamily: 'var(--mono)',
  fontSize: 10.5,
  color: 'var(--muted)',
}

interface AuditItem {
  ts: string
  txt: string
  hash: string
}

/** U5 · Squad Designer — construído SOBRE a lib agnóstica `danzeroum/bpmn`
 *  (submodule `vendor/bpmn`): `BpmnEditor` (canvas/gestos/paleta/inspetor/
 *  undo/lifecycle) + plugin de domínio do produto (`btvPlugin`). A auditoria
 *  do fluxo usa o `AuditLedger` hash-chained da lib; salvar registra a
 *  versão no `VersionRegistry` (lifecycle real) e espelha no ledger BuildToValue
 *  (`btv.flow_saved`); ▶ Testar roda o fluxo no motor REAL do squad, com o
 *  run-binding (`versionId`/`snapshotHash`) na trilha. */
export function Designer() {
  const templates = useTemplates()
  const { ativarTeste } = useSquadRun()
  const [nome, setNome] = useState('Estudo de caso')
  const [base, setBase] = useState<'inicial' | 'blank' | string>('inicial')
  const [diagram, setDiagram] = useState<BpmnDiagram>(() => baseInicial())
  const [editorKey, setEditorKey] = useState(0)
  const [audit, setAudit] = useState<AuditItem[]>([])
  const [salvo, setSalvo] = useState<string | null>(null)
  const [erro, setErro] = useState<string | null>(null)
  const ledgerRef = useRef<AuditLedger>(null)
  if (!ledgerRef.current) ledgerRef.current = new AuditLedger()
  const registryRef = useRef<VersionRegistry>(null)
  if (!registryRef.current) registryRef.current = new VersionRegistry()

  const registrar = (tipo: string, txt: string) => {
    void ledgerRef.current!
      .append({ type: tipo, userId: 'você', versionId: diagram.version.id, details: { txt } })
      .then((entry) => {
        const ts = entry.timestamp.match(/T(\d{2}:\d{2}:\d{2})/)?.[1] ?? ''
        setAudit((a) => [{ ts, txt, hash: entry.hash.slice(0, 8) }, ...a])
      })
  }

  const trocarBase = (novaBase: string, d: BpmnDiagram, msg: string) => {
    setBase(novaBase)
    setDiagram(d)
    setEditorKey((k) => k + 1)
    setSalvo(null)
    registrar('flow.base', msg)
  }

  const etapas = useMemo(() => etapasDoFluxo(diagram), [diagram])
  const ordem = useMemo(() => ordemDoFluxo(diagram), [diagram])

  const salvar = async () => {
    setErro(null)
    try {
      const registro = await registryRef.current!.register(diagram, {
        technicalNotes: `salvo pelo Squad Designer como "${nome}"`,
      })
      const head = ledgerRef.current!.getEntries().at(-1)
      const resp = await fetchJson<{ seq: number; diagram_sha256: string }>(
        '/api/btv/designer/flows',
        {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            nome,
            diagram,
            versao_semantica: diagram.version.semanticVersion,
            snapshot_hash: registro.snapshotHash,
            audit_head: head?.hash,
            audit_len: ledgerRef.current!.getEntries().length,
          }),
        },
      )
      setSalvo(`✓ salvo e auditado — ledger #${resp.seq}`)
      registrar('flow.saved', `fluxo salvo como modelo "${nome}" (ledger da plataforma #${resp.seq})`)
    } catch (e) {
      setErro(e instanceof Error ? e.message : String(e))
    }
  }

  const testar = async () => {
    setErro(null)
    if (etapas.length < 2) {
      setErro('desenhe ao menos um bloco antes de testar')
      return
    }
    try {
      // Run-binding (§12): a execução nasce presa à versão exata do fluxo.
      const registro = await registryRef.current!.register(diagram, {
        technicalNotes: `teste da squad "${nome}"`,
      })
      registrar(
        'flow.test',
        `execução de teste iniciada — ${etapas.length - 1} etapas · snapshot ${registro.snapshotHash.slice(0, 8)}`,
      )
      const descricao = `${descricaoDoFluxo(nome, diagram)}\n\n(run-binding: versão ${diagram.version.semanticVersion}, snapshot ${registro.snapshotHash})`
      await ativarTeste(nome, etapas, descricao)
    } catch (e) {
      setErro(e instanceof Error ? e.message : String(e))
    }
  }

  return (
    <>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
        <input
          value={nome}
          onChange={(e) => setNome(e.target.value)}
          title="nome da squad"
          style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 15, color: 'var(--ink)', background: 'none', border: 'none', borderBottom: '1px dashed var(--line2)', padding: '4px 2px', width: 200 }}
        />
        <span className="kicker" style={{ fontSize: 10, color: 'var(--faint)' }}>começar de</span>
        {[
          ['inicial', 'Estudo de caso', () => trocarBase('inicial', baseInicial(), 'fluxo inicial carregado — 5 blocos, 4 setas')],
          ['blank', 'Em branco', () => trocarBase('blank', baseVazia(), 'fluxo zerado — base em branco')],
          ...(templates.status === 'ready'
            ? (['editorial', 'pesquisa', 'musica'] as const).map((id) => {
                const t = templates.byId.get(id)!
                return [
                  id,
                  t.nome.split(' /')[0].split(' &')[0],
                  () => {
                    setNome(`Derivada de ${t.nome}`)
                    trocarBase(id, baseDoModelo(t), `base carregada: ${t.nome}`)
                  },
                ] as [string, string, () => void]
              })
            : []),
        ].map(([id, label, on]) => (
          <button
            key={id as string}
            onClick={on as () => void}
            style={{
              borderRadius: 999,
              padding: '6px 13px',
              fontSize: 11.5,
              fontWeight: 600,
              fontFamily: 'var(--sans)',
              border: `1px solid ${base === id ? 'var(--brand)' : 'var(--line2)'}`,
              background: base === id ? 'var(--brand)' : 'var(--white)',
              color: base === id ? '#fff' : 'var(--muted)',
            }}
          >
            {label as string}
          </button>
        ))}
        <span style={{ marginLeft: 'auto', display: 'flex', gap: 8, alignItems: 'center' }}>
          {erro && <span style={{ fontSize: 11, color: 'var(--err-ink)', maxWidth: 260 }}>{erro}</span>}
          <button onClick={() => void salvar()} style={{ ...ghost, color: 'var(--brand)', fontWeight: 600 }}>
            {salvo ?? 'salvar como modelo'}
          </button>
          <button
            onClick={() => void testar()}
            style={{ background: 'var(--brand)', color: '#fff', border: 'none', borderRadius: 9, padding: '9px 18px', fontSize: 12.5, fontWeight: 600, fontFamily: 'var(--sans)' }}
          >
            ▶ Testar squad
          </button>
        </span>
      </div>

      <div
        data-testid="designer-canvas"
        style={{ border: '1px solid var(--line)', borderRadius: 14, background: 'var(--white)', height: 500, overflow: 'hidden' }}
      >
        <BpmnEditor
          key={editorKey}
          diagram={diagram}
          plugins={[btvDesignerPlugin]}
          onChange={setDiagram}
          hideMiniMap
        />
      </div>

      <div style={{ display: 'flex', gap: 7, alignItems: 'center', flexWrap: 'wrap', background: 'var(--card)', border: '1px solid var(--line)', borderRadius: 12, padding: '12px 16px' }}>
        <span className="kicker" style={{ fontSize: 10, color: 'var(--faint)', marginRight: 6 }}>esteira resultante</span>
        <span className="mono" style={{ fontSize: 11, fontWeight: 600, background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 999, padding: '5px 11px' }}>
          ⌂ Briefing
        </span>
        {ordem.map((n, i) => (
          <span key={n.id} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ color: 'var(--faint)', fontSize: 11 }}>→</span>
            <span className="mono" style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, fontWeight: 600, background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 999, padding: '5px 11px' }}>
              <span style={{ width: 7, height: 7, borderRadius: 2, background: BLOCO_META[n.type]?.cor ?? 'var(--faint)' }} />
              {BLOCO_META[n.type]?.icon ?? '•'} {n.label}
            </span>
            {i === ordem.length - 1 && null}
          </span>
        ))}
      </div>

      <div data-testid="auditoria-fluxo" style={{ background: 'var(--card)', border: '1px solid var(--line)', borderRadius: 12, padding: '12px 16px', display: 'flex', flexDirection: 'column', gap: 6 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span className="kicker" style={{ fontSize: 10, color: 'var(--faint)' }}>auditoria do fluxo</span>
          <span className="mono" style={{ fontSize: 9.5, color: 'var(--faint)', marginLeft: 'auto' }}>
            {audit.length} eventos · cadeia SHA-256 da lib · espelhada no ledger ao salvar
          </span>
        </div>
        {audit.length === 0 && (
          <span className="mono" style={{ fontSize: 10.5, color: 'var(--faint)' }}>
            mutações do fluxo aparecem aqui (cadeia hash-encadeada do AuditLedger)
          </span>
        )}
        {audit.slice(0, 8).map((l, i) => (
          <div key={i} className="mono" style={{ display: 'flex', gap: 10, fontSize: 10.5, color: 'var(--muted)', borderTop: '1px dashed var(--line)', paddingTop: 6 }}>
            <span style={{ color: 'var(--faint)', flex: 'none' }}>{l.ts}</span>
            <span style={{ flex: 1 }}>{l.txt}</span>
            <span style={{ color: 'var(--faint)' }}>{l.hash}</span>
          </div>
        ))}
      </div>
    </>
  )
}
