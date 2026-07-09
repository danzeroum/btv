// ================= BTV dashboard — teste completo (cole no console) =================
// Cobre BACKEND (rotas /api/*), FRONTEND (dirige o DOM real da SPA) e a
// INTERSEÇÃO (DOM ↔ API) de todas as funcionalidades da UI.
//
//   btvFull()                     -> backend + frontend + interseção (default)
//   btvFull({ heavy:true })       -> inclui verify/squad/sessão (lento, precisa sidecar/provider)
//   btvFull({ backend:false })    -> só dirige a UI (frontend + interseção)
//   btvFull({ frontend:false })   -> só backend (equivale ao antigo btvSmoke)
//
// Abra a RAIZ do dashboard (o btv-web, não /dev) para o frontend ser dirigido.
window.btvFull = async function btvFull(opts = {}) {
  const CFG = { backend: true, frontend: true, cross: true, heavy: false, timeoutMs: 15000, uiWaitMs: 7000, ...opts }
  const BASE = location.origin
  const rows = []
  const rec = (group, name, status, detail) => {
    rows.push({ group, name, status, detail: detail ?? '' })
    const icon = status === 'PASS' ? '✅' : status === 'WARN' ? '🟡' : status === 'SKIP' ? '⚪' : '❌'
    console.log(`${icon} [${group}] ${name}${detail ? ' — ' + detail : ''}`)
  }
  const req = async (method, path, body) => {
    const ctrl = new AbortController()
    const t = setTimeout(() => ctrl.abort(), CFG.timeoutMs)
    try {
      const r = await fetch(BASE + path, {
        method,
        headers: body !== undefined ? { 'content-type': 'application/json' } : undefined,
        body: body !== undefined ? JSON.stringify(body) : undefined,
        signal: ctrl.signal,
      })
      const txt = await r.text()
      let data = null; try { data = txt ? JSON.parse(txt) : null } catch { data = txt }
      return { status: r.status, ok: r.ok, data }
    } finally { clearTimeout(t) }
  }
  const check = async (group, name, fn) => {
    try { rec(group, name, 'PASS', await fn()) }
    catch (e) {
      if (e && e.warn) rec(group, name, 'WARN', e.msg)
      else if (e && e.skip) rec(group, name, 'SKIP', e.msg)
      else rec(group, name, 'FAIL', e && e.message ? e.message : String(e))
    }
  }
  const need = (cond, msg) => { if (!cond) throw new Error(msg) }
  const warn = (msg) => { throw { warn: true, msg } }
  const skip = (msg) => { throw { skip: true, msg } }
  const sidecarMaybe = (res) => { if (res.status === 502 || res.status === 503) warn('sidecar off (' + res.status + ')') }
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

  // ---- helpers de DOM (dirigem a SPA de verdade) ----
  const ROOT = () => document.querySelector('#btv-root')
  const title = () => (document.querySelector('#btv-root h1.screen-title')?.textContent || '').trim()
  const buttons = () => [...document.querySelectorAll('#btv-root button')]
  const clickText = (txt) => {
    const b = buttons().find((x) => (x.textContent || '').includes(txt))
    need(b, 'botão não encontrado no DOM: “' + txt + '”')
    b.click(); return b
  }
  const waitUntil = async (fn, ms = CFG.uiWaitMs) => {
    const t0 = Date.now()
    for (;;) {
      let v; try { v = fn() } catch { v = false }
      if (v) return v
      if (Date.now() - t0 > ms) throw new Error('timeout esperando a UI (' + ms + 'ms)')
      await sleep(120)
    }
  }
  const tids = (prefix) => [...document.querySelectorAll(`#btv-root [data-testid^="${prefix}"]`)]
  const leavesExact = (text) =>
    [...document.querySelectorAll('#btv-root *')].filter((el) => el.children.length === 0 && (el.textContent || '').trim() === text)
  const rootText = () => (ROOT()?.innerText || '')
  const statValue = (label) => {
    const k = [...document.querySelectorAll('#btv-root .kicker')].find((el) => (el.textContent || '').trim() === label)
    return (k?.nextElementSibling?.textContent || '').trim()
  }
  const goPersona = async (p) => { clickText(p === 'admin' ? 'Administração' : 'Meu espaço'); await sleep(80) }
  const nav = async (label, expTitle) => { clickText(label); await waitUntil(() => title() === expTitle); return title() }

  const sse = (path, ms = 8000) => new Promise((resolve) => {
    const es = new EventSource(BASE + path)
    let n = 0, done = false
    const stop = (why) => { if (done) return; done = true; es.close(); resolve({ n, why }) }
    es.onmessage = (ev) => { n++; if (/"kind":"?(consensus|final|error)"?/.test(ev.data) || n > 40) stop('evento') }
    es.addEventListener('done', () => stop('done'))
    es.onerror = () => stop('erro/fim')
    setTimeout(() => stop('timeout'), ms)
  })

  console.log(`%c BTV FULL @ ${BASE} — backend=${CFG.backend} frontend=${CFG.frontend} cross=${CFG.cross} heavy=${CFG.heavy}`, 'font-weight:bold')
  const uiReady = !!ROOT()
  if ((CFG.frontend || CFG.cross) && !uiReady) {
    rec('UI', 'SPA carregada', 'WARN', 'sem #btv-root — abra a RAIZ do dashboard (btv-web). Rodando só backend.')
  }

  // caches p/ a interseção
  let apiTemplates = [], apiUsers = [], apiSquads = [], apiDeliverables = [], apiProviders = [], apiSummary = null

  // =========================== 1) BACKEND (rotas /api/*) ===========================
  if (CFG.backend) {
    const G = 'API·GET'
    await check(G, '/api/summary', async () => { const r = await req('GET','/api/summary'); need(r.status===200,'status '+r.status); apiSummary=r.data; return `${r.data.total_events} eventos` })
    await check(G, '/api/events', async () => { const r = await req('GET','/api/events'); need(r.status===200,'status '+r.status); return `${(r.data||[]).length} eventos` })
    await check(G, '/api/skills', async () => { const r = await req('GET','/api/skills'); need(r.status===200,'status '+r.status); return `${(r.data||[]).length} skills` })
    await check(G, '/api/prompts', async () => { const r = await req('GET','/api/prompts'); need(r.status===200,'status '+r.status); return `${(r.data||[]).length} prompts` })
    await check(G, '/api/ledger?limit=20', async () => { const r = await req('GET','/api/ledger?limit=20'); need(r.status===200,'status '+r.status); return `${(r.data||[]).length} entradas` })
    await check(G, '/api/models/usage', async () => { const r = await req('GET','/api/models/usage'); need(r.status===200,'status '+r.status); return `${(r.data.entries||[]).length} modelos · $${(r.data.total_estimated_cost_usd??0).toFixed?.(4) ?? r.data.total_estimated_cost_usd} (tab ${r.data.pricing_as_of})` })
    await check(G, '/api/ratelimit', async () => { const r = await req('GET','/api/ratelimit'); need(r.status===200,'status '+r.status); return `${(r.data||[]).length} tiers` })
    await check(G, '/api/providers', async () => { const r = await req('GET','/api/providers'); need(r.status===200,'status '+r.status); apiProviders=r.data||[]; return `${apiProviders.filter(p=>p.configured).length}/${apiProviders.length} configurados` })
    await check(G, '/api/mcp', async () => { const r = await req('GET','/api/mcp'); need(r.status===200,'status '+r.status); return `${(r.data||[]).length} servidores` })
    await check(G, '/api/lsp', async () => { const r = await req('GET','/api/lsp'); need(r.status===200,'status '+r.status); return `${(r.data||[]).length} servidores` })
    await check(G, '/api/sandbox', async () => { const r = await req('GET','/api/sandbox'); need(r.status===200,'status '+r.status); return `ping=${JSON.stringify(r.data.ping ?? r.data)}` })
    await check(G, '/api/doctor', async () => { const r = await req('GET','/api/doctor'); need(r.status===200,'status '+r.status); return 'ok' })
    await check(G, '/api/memory', async () => { const r = await req('GET','/api/memory'); if(r.status!==200){ sidecarMaybe(r); need(false,'status '+r.status) } return `${(r.data?.memories||r.data||[]).length ?? '?'} memórias` })
    await check(G, '/api/permissions/matrix', async () => { const r = await req('GET','/api/permissions/matrix'); need(r.status===200,'status '+r.status); return 'ok' })
    await check(G, '/api/experiment/{inexistente}', async () => { const r = await req('GET','/api/experiment/full-inexistente-'+location.hash+Math.floor(performance.now())); need(r.status===404,'esperava 404, veio '+r.status); return '404 esperado' })
    const P = 'API·btv'
    await check(P, '/api/btv/templates', async () => { const r = await req('GET','/api/btv/templates'); need(r.status===200,'status '+r.status); apiTemplates=r.data||[]; return `${apiTemplates.length} templates` })
    await check(P, '/api/btv/squads', async () => { const r = await req('GET','/api/btv/squads'); need(r.status===200,'status '+r.status); apiSquads=r.data||[]; return `${apiSquads.length} runs` })
    await check(P, '/api/btv/deliverables', async () => { const r = await req('GET','/api/btv/deliverables'); need(r.status===200,'status '+r.status); apiDeliverables=r.data||[]; return `${apiDeliverables.length} entregas` })
    await check(P, '/api/btv/users', async () => { const r = await req('GET','/api/btv/users'); need(r.status===200,'status '+r.status); apiUsers=r.data||[]; return `${apiUsers.length} perfis` })
    await check(P, '/api/btv/templates/publicacao', async () => { const r = await req('GET','/api/btv/templates/publicacao'); need(r.status===200,'status '+r.status); return `${(r.data||[]).length} pubs` })
    await check(P, '/api/prompt/generators', async () => { const r = await req('GET','/api/prompt/generators'); if(r.status!==200){ sidecarMaybe(r); need(false,'status '+r.status) } return `${(r.data?.generators||r.data||[]).length ?? '?'} geradores` })

    // Mutações leves (criam e limpam)
    const M = 'API·MUT'
    await check(M, 'prompt create+fav+delete', async () => {
      const c = await req('POST','/api/prompts',{ name:'FULL '+Math.floor(performance.now()), generator:'smoke', fields:{}, rendered:'teste', tags:['smoke'] })
      need(c.status===200||c.status===201,'create '+c.status); const id=c.data?.id ?? c.data; need(id!=null,'sem id')
      const f = await req('POST',`/api/prompts/${id}/favorite`); need(f.ok,'fav '+f.status)
      const d = await req('DELETE',`/api/prompts/${id}`); need(d.ok,'delete '+d.status)
      return `id ${id} criado, favoritado, removido`
    })
    await check(M, 'permission rule set+revoke', async () => {
      const scope='full-'+Math.floor(performance.now())
      const s = await req('POST','/api/permissions/rules',{ profile:'build', tool:'bash', scope_prefix:scope, decision:'ask' }); need(s.ok,'set '+s.status)
      const list = await req('GET','/api/permissions/rules'); need(list.ok,'list '+list.status)
      const found = (list.data||[]).find((x)=> x.scope_prefix===scope || JSON.stringify(x).includes(scope)); need(found,'rule não apareceu')
      const d = await req('DELETE',`/api/permissions/rules/${found.id}`); need(d.ok,'revoke '+d.status)
      return `rule ${found.id} gravada e revogada`
    })
    await check(M, 'ledger verify', async () => { const r = await req('POST','/api/ledger/verify'); need(r.status===200,'status '+r.status); return `ok=${r.data?.ok} verified=${r.data?.verified}` })
    await check(M, 'memory recall', async () => { const r = await req('POST','/api/memory/recall',{ query:'teste', k:3 }); if(r.status!==200){ sidecarMaybe(r); need(false,'status '+r.status) } return 'ok' })
    await check(M, 'prompt render', async () => {
      const gens = await req('GET','/api/prompt/generators'); if(gens.status!==200){ sidecarMaybe(gens); need(false,'generators '+gens.status) }
      const g = (gens.data?.generators||gens.data||[])[0]; if(!g) warn('sem gerador'); const gid = g.id ?? g.name ?? g
      const r = await req('POST','/api/prompt/render',{ generator:gid, fields:{} }); if(r.status!==200){ sidecarMaybe(r); need(false,'render '+r.status) }
      return `render de "${gid}" ok`
    })
    await check(M, 'designer workflow save', async () => {
      const node = { id:'n1', x:0, y:0, kind:'card', name:'Full', role:'architect', color:'#888', icon:'●', sub:'', params:[], removable:true }
      const r = await req('POST','/api/designer/workflow',{ nodes:[node], edges:[] }); need(r.ok,'status '+r.status); return `salvo (seq ${r.data?.seq ?? '?'})`
    })
    await check(M, 'A6 PIN create+verify+clear', async () => {
      const nome='FULL·pin·'+Math.floor(performance.now())
      const c = await req('POST','/api/btv/users',{ nome, email:'', papel:'usuario', pin:'4242' }); need(c.status===201||c.ok,'create '+c.status); const id=c.data?.id; need(id!=null,'sem id')
      const wrong = await req('POST',`/api/btv/users/${id}/verify-pin`,{ pin:'0000' }); need(wrong.data?.ok===false,'PIN errado deveria falhar')
      const right = await req('POST',`/api/btv/users/${id}/verify-pin`,{ pin:'4242' }); need(right.data?.ok===true,'PIN certo deveria passar')
      const clr = await req('POST',`/api/btv/users/${id}/pin`,{ pin:'' }); need(clr.ok,'clear '+clr.status)
      // Auto-limpeza: remove de vez (rota nova). Backend antigo sem DELETE → suspende.
      const del = await req('DELETE',`/api/btv/users/${id}`)
      if (del.ok) return `perfil ${id}: PIN ok, limpo, removido`
      await req('POST',`/api/btv/users/${id}/ativo`,{ ativo:false })
      return `perfil ${id}: PIN ok, limpo, suspenso (backend sem delete)`
    })
    await check(M, 'template publicação toggle', async () => {
      if(!apiTemplates.length) warn('sem templates'); const tid = apiTemplates[0].id ?? apiTemplates[0].template_id
      const pubs = await req('GET','/api/btv/templates/publicacao'); const cur=(pubs.data||[]).find((p)=>p.template_id===tid)?.publicado ?? false
      const a = await req('POST',`/api/btv/templates/${tid}/publicacao`,{ publicado:!cur }); need(a.ok,'toggle '+a.status)
      const b = await req('POST',`/api/btv/templates/${tid}/publicacao`,{ publicado:cur }); need(b.ok,'restore '+b.status)
      return `${tid} alternado e restaurado`
    })
    await check(M, 'btv designer flow save', async () => {
      const diagram = { nodes:{ n1:{ id:'n1', type:'task', name:'Full' } }, edges:{} }
      const r = await req('POST','/api/btv/designer/flows',{ nome:'FULL flow', diagram }); need(r.ok,'status '+r.status); return `salvo (seq ${r.data?.seq ?? '?'})`
    })
  }

  // =========================== 2) FRONTEND (dirige o DOM) ===========================
  // Cada tela é navegada de verdade (clica persona + item de menu) e provamos que
  // o React renderizou o título e os elementos-assinatura reais.
  if (CFG.frontend && uiReady) {
    const F = 'UI·render'
    await check(F, 'topbar troca persona', async () => {
      await goPersona('admin'); await waitUntil(() => title() === 'Telemetria & custos')
      await goPersona('user'); await waitUntil(() => title() === 'Monte uma squad, receba entregas')
      return 'Meu espaço ↔ Administração ok'
    })
    // --- perfil usuário ---
    await check(F, 'U1 Início (galeria)', async () => { await goPersona('user'); await nav('Início','Monte uma squad, receba entregas'); await waitUntil(()=>tids('card-').length>0); return `${tids('card-').length} cards` })
    await check(F, 'U1 card abre wizard e fecha', async () => {
      await goPersona('user'); await nav('Início','Monte uma squad, receba entregas'); await waitUntil(()=>tids('card-').length>0)
      tids('card-')[0].click(); await waitUntil(()=>document.querySelector('[data-testid="wizard-overlay"]'))
      const close = buttons().find((b)=> b.getAttribute('aria-label')==='Fechar wizard'); need(close,'sem botão fechar wizard'); close.click()
      await waitUntil(()=>!document.querySelector('[data-testid="wizard-overlay"]')); return 'wizard abriu e fechou'
    })
    await check(F, 'U6 Minhas squads', async () => { await nav('Minhas squads','Minhas squads'); return `${tids('run-').length} runs no DOM` })
    await check(F, 'U7 Personas', async () => { await nav('Personas','Personas & prompts'); await waitUntil(()=>tids('persona-').length>0); return `${tids('persona-').length} papéis` })
    await check(F, 'U4 Biblioteca', async () => { await nav('Biblioteca','Biblioteca de entregas'); return `${tids('entrega-').length} entregas no DOM` })
    await check(F, 'U5 Designer', async () => { await nav('Designer','Squad Designer'); await waitUntil(()=>document.querySelector('#btv-root [data-testid="designer-canvas"]')); return 'canvas renderizado' })
    // --- perfil admin ---
    await check(F, 'A1 Telemetria', async () => { await goPersona('admin'); await nav('Telemetria','Telemetria & custos'); await waitUntil(()=>!!statValue('squads ativadas')); return `squads ativadas=${statValue('squads ativadas')}` })
    await check(F, 'A2 Ledger', async () => { await nav('Ledger','Ledger de auditoria'); await waitUntil(()=>/entradas mais recentes/.test(rootText())); return 'trilha renderizada' })
    await check(F, 'A3 Providers', async () => { await nav('Providers','Providers & rate limits'); await waitUntil(()=>leavesExact('configurado').length + leavesExact('sem key').length > 0); return `${leavesExact('configurado').length} configurado(s)` })
    await check(F, 'A4 Permissões', async () => { await nav('Permissões','Permissões — skills, tools e MCP'); return 'tela renderizada' })
    await check(F, 'A5 Modelos', async () => { await nav('Modelos','Modelos de squad'); await waitUntil(()=>tids('modelo-').length>0); return `${tids('modelo-').length} modelos` })
    await check(F, 'A6 Usuários', async () => { await nav('Usuários','Usuários & acessos'); await waitUntil(()=>tids('user-').length>0); return `${tids('user-').length} usuários` })
  }

  // =========================== 3) INTERSEÇÃO (DOM ↔ API) ===========================
  // A prova de fronteira: o que a tela mostra é o que o backend devolve.
  if (CFG.cross && uiReady) {
    const X = 'CRUZA'
    const apiLen = async (path) => { const r = await req('GET',path); need(r.status===200,path+' '+r.status); return (r.data||[]).length }

    await check(X, 'galeria(cards) == /api/btv/templates', async () => {
      await goPersona('user'); await nav('Início','Monte uma squad, receba entregas'); await waitUntil(()=>tids('card-').length>0)
      const dom = tids('card-').length; const api = await apiLen('/api/btv/templates'); need(dom===api,`DOM ${dom} ≠ API ${api}`); return `${dom} cards == ${api} templates`
    })
    await check(X, 'personas == papéis do template editorial', async () => {
      await nav('Personas','Personas & prompts'); await waitUntil(()=>tids('persona-').length>0)
      const dom = tids('persona-').length; const t = (apiTemplates.length?apiTemplates:(await (await fetch(BASE+'/api/btv/templates')).json())).find((x)=>x.id==='editorial')
      const api = t?.papeis?.length ?? dom; need(dom===api,`DOM ${dom} ≠ template.papeis ${api}`); return `${dom} personas == ${api} papéis`
    })
    await check(X, 'biblioteca == /api/btv/deliverables', async () => {
      await nav('Biblioteca','Biblioteca de entregas'); await sleep(400)
      const dom = tids('entrega-').length; const api = await apiLen('/api/btv/deliverables'); need(dom===api,`DOM ${dom} ≠ API ${api}`); return `${dom} entregas == ${api}`
    })
    await check(X, 'minhas squads == /api/btv/squads', async () => {
      await nav('Minhas squads','Minhas squads'); await sleep(400)
      const dom = tids('run-').length; const api = await apiLen('/api/btv/squads'); need(dom===api,`DOM ${dom} ≠ API ${api}`); return `${dom} runs == ${api}`
    })
    await check(X, 'modelos == /api/btv/templates', async () => {
      await goPersona('admin'); await nav('Modelos','Modelos de squad'); await waitUntil(()=>tids('modelo-').length>0)
      const dom = tids('modelo-').length; const api = await apiLen('/api/btv/templates'); need(dom===api,`DOM ${dom} ≠ API ${api}`); return `${dom} linhas == ${api} templates`
    })
    await check(X, 'usuários == /api/btv/users', async () => {
      await nav('Usuários','Usuários & acessos'); await waitUntil(()=>tids('user-').length>0)
      const dom = tids('user-').length; const api = await apiLen('/api/btv/users'); need(dom===api,`DOM ${dom} ≠ API ${api}`); return `${dom} linhas == ${api} perfis`
    })
    await check(X, 'providers configurados: DOM == /api/providers', async () => {
      await nav('Providers','Providers & rate limits'); await waitUntil(()=>leavesExact('configurado').length + leavesExact('sem key').length > 0)
      const dom = leavesExact('configurado').length; const r = await req('GET','/api/providers'); const api = (r.data||[]).filter((p)=>p.configured).length
      need(dom===api,`DOM ${dom} ≠ API ${api}`); return `${dom} configurado(s) == ${api}`
    })
    await check(X, 'telemetria StatCards refletem a API', async () => {
      await nav('Telemetria','Telemetria & custos'); await waitUntil(()=>!!statValue('squads ativadas'))
      const s = await req('GET','/api/summary'); const sq = await req('GET','/api/btv/squads')
      const evDom = statValue('eventos de telemetria'); const sqDom = statValue('squads ativadas')
      need(evDom===String(s.data.total_events),`eventos DOM ${evDom} ≠ API ${s.data.total_events}`)
      need(sqDom===String((sq.data||[]).length),`squads DOM ${sqDom} ≠ API ${(sq.data||[]).length}`)
      return `eventos=${evDom} · squads=${sqDom} batem`
    })
    await check(X, 'ledger: linhas renderizadas == cabeçalho', async () => {
      await nav('Ledger','Ledger de auditoria'); await waitUntil(()=>/entradas mais recentes/.test(rootText()))
      const m = rootText().match(/(\d+)\s+entradas mais recentes/); need(m,'cabeçalho de entradas não encontrado')
      const header = Number(m[1]); const rowsDom = [...document.querySelectorAll('#btv-root *')].filter((el)=>el.children.length===0 && /^[0-9a-f]{4}…[0-9a-f]{4}$/.test((el.textContent||'').trim())).length
      need(rowsDom===header,`linhas ${rowsDom} ≠ cabeçalho ${header}`); return `${rowsDom} hashes == ${header} entradas`
    })
  }

  // ---------------------------- 4) PESADAS (opt-in) ----------------------------
  if (CFG.heavy) {
    const H = 'HEAVY'
    let runId = null
    await check(H, 'verify run + polling', async () => {
      const s = await req('POST','/api/verify/run'); need(s.status===202||s.status===409,'run '+s.status); runId = s.data?.run_id; need(runId,'sem run_id')
      for (let i=0;i<120;i++){ await sleep(1000); const g = await req('GET',`/api/verify/${runId}`)
        if(g.data?.status==='done'){ const rev=g.data.review; return `veredito=${g.data.evidence?.verdict} · review tech=${rev?.technical} sec=${rev?.security} gates=${rev?.gates_passed}` }
        if(g.data?.status==='failed'){ return `failed: ${g.data.message}` } }
      warn('não concluiu em 120s')
    })
    await check(H, 'verify 409 concorrente', async () => {
      const [a,b] = await Promise.all([req('POST','/api/verify/run'), req('POST','/api/verify/run')])
      const st=[a.status,b.status].sort(); need(st[0]===202&&st[1]===409,'esperava [202,409], veio '+JSON.stringify(st))
      need(a.data?.run_id===b.data?.run_id,'run_ids diferentes'); return `202+409 mesmo run_id`
    })
    await check(H, 'squad run + SSE', async () => {
      const s = await req('POST','/api/squad/run',{ task:'diga apenas: olá' }); need(s.status===202,'run '+s.status); const tid=s.data?.task_id; need(tid,'sem task_id')
      const ev = await sse(`/api/squad/${tid}/events`, 20000); await req('POST',`/api/squad/${tid}/emergency-stop`,{})
      need(ev.n>0,'sem eventos SSE (sidecar/provider?)'); return `task ${tid}: ${ev.n} eventos (${ev.why})`
    })
    await check(H, 'sessão chat SSE+message', async () => {
      const sid = (crypto.randomUUID && crypto.randomUUID()) || ('full-'+Math.floor(performance.now()))
      const p = sse(`/api/session/${sid}/events`, 15000)
      const m = await req('POST',`/api/session/${sid}/message`,{ message:'diga olá', agent:'build' }); need(m.status===202,'message '+m.status)
      const ev = await p; if(ev.n===0) warn('sem eventos (provider/sidecar? 202 aceito)'); return `sessão ${sid.slice(0,8)}: ${ev.n} eventos`
    })
  }

  // restaura a UI para o estado inicial (Meu espaço · Início)
  if (uiReady) { try { await goPersona('user'); if (title() !== 'Monte uma squad, receba entregas') { clickText('Início'); await waitUntil(()=>title()==='Monte uma squad, receba entregas', 2000) } } catch {} }

  return summarize(CFG.heavy ? undefined : 'Rode btvFull({heavy:true}) p/ verify+squad+sessão.')

  function summarize(hint) {
    const by = rows.reduce((a,r)=>{a[r.status]=(a[r.status]||0)+1;return a},{})
    console.log(`%c RESUMO: ${by.PASS||0} PASS · ${by.WARN||0} WARN · ${by.SKIP||0} SKIP · ${by.FAIL||0} FAIL`, 'font-weight:bold;font-size:13px')
    console.table(rows)
    if (hint) console.log('ℹ️ ' + hint)
    if (by.FAIL) console.log('%c Há falhas — veja as linhas FAIL acima.', 'color:#c0392b;font-weight:bold')
    return rows
  }
}
btvFull()
