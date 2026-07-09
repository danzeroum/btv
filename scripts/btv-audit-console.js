// ============ BTV dashboard — AUDITORIA ADVERSARIAL (cole no console) ============
// Complementa o btvFull(): em vez de confirmar o caminho feliz, tenta QUEBRAR a
// app e caçar DECISÕES SILENCIOSAS. Rótulos:
//   ✅ PASS  a defesa esperada existe (bug NÃO reproduzido)
//   ❌ FAIL  bug real: 500 em input inválido, dado fabricado, guarda furada
//   🟡 WARN  decisão silenciosa / suspeita — olhar humano
//   ℹ️ INFO  observação honesta (comportamento por design, mas vale registrar)
//
//   btvAudit()                  -> API + UI (não-intrusivo; sem resíduo)
//   btvAudit({ intrusive:true })-> inclui sonda de XSS-armazenado (cria 1 perfil residual)
//   btvAudit({ ui:false })      -> só API
window.btvAudit = async function btvAudit(opts = {}) {
  const CFG = { api: true, ui: true, intrusive: false, timeoutMs: 15000, uiWaitMs: 7000, ...opts }
  const BASE = location.origin
  const rows = []
  const rec = (group, name, status, detail) => {
    rows.push({ group, name, status, detail: detail ?? '' })
    const icon = { PASS:'✅', FAIL:'❌', WARN:'🟡', INFO:'ℹ️' }[status] || '·'
    const css = status==='FAIL' ? 'color:#c0392b;font-weight:bold' : status==='WARN' ? 'color:#b9770e' : status==='INFO' ? 'color:#5566aa' : 'color:#2d6a50'
    console.log(`%c${icon} [${group}] ${name}${detail?' — '+detail:''}`, css)
  }
  const req = async (method, path, body, rawBody) => {
    const ctrl = new AbortController(); const t = setTimeout(()=>ctrl.abort(), CFG.timeoutMs)
    try {
      const r = await fetch(BASE + path, {
        method,
        headers: (body!==undefined||rawBody!==undefined) ? { 'content-type':'application/json' } : undefined,
        body: rawBody!==undefined ? rawBody : (body!==undefined ? JSON.stringify(body) : undefined),
        signal: ctrl.signal,
      })
      const txt = await r.text(); let data=null; try{ data = txt?JSON.parse(txt):null }catch{ data=txt }
      return { status:r.status, ok:r.ok, data }
    } finally { clearTimeout(t) }
  }
  const P = (g,n,d)=>rec(g,n,'PASS',d), F=(g,n,d)=>rec(g,n,'FAIL',d), W=(g,n,d)=>rec(g,n,'WARN',d), I=(g,n,d)=>rec(g,n,'INFO',d)
  const sleep = (ms)=>new Promise(r=>setTimeout(r,ms))
  const near = (a,b,eps=1e-9)=>Math.abs(a-b)<=eps

  console.log(`%c BTV AUDIT @ ${BASE} — api=${CFG.api} ui=${CFG.ui} intrusive=${CFG.intrusive}`, 'font-weight:bold;font-size:13px')

  // ===================== 1) VALIDAÇÃO & FRONTEIRAS (input hostil) =====================
  // Regra de ouro: input inválido pode dar 4xx, NUNCA 500 nem 200 silencioso.
  if (CFG.api) {
    const V = 'VALIDA'
    const expect4xxNot500 = async (name, method, path, body, rawBody) => {
      const r = await req(method, path, body, rawBody)
      if (r.status === 500) return F(V, name, `500 em input inválido (bug): ${JSON.stringify(r.data).slice(0,120)}`)
      if (r.status >= 400 && r.status < 500) return P(V, name, `rejeitado com ${r.status} ${r.data?.error ?? ''}`)
      return W(V, name, `aceitou input inválido: ${r.status} (esperado 4xx)`)
    }
    await expect4xxNot500('user create nome vazio', 'POST', '/api/btv/users', { nome:'   ', email:'', papel:'usuario' })
    await expect4xxNot500('user create corpo {}', 'POST', '/api/btv/users', {})
    await expect4xxNot500('designer flow nodes=array', 'POST', '/api/btv/designer/flows', { nome:'x', diagram:{ nodes:[], edges:{} } })
    await expect4xxNot500('verify-pin sem campo pin', 'POST', '/api/btv/users/1/verify-pin', {})
    await expect4xxNot500('JSON malformado', 'POST', '/api/btv/users', undefined, '{ not json ')
    // IDs inexistentes: pin/verify-pin distinguem 404; ativo NÃO (no-op silencioso)
    await (async()=>{ const r = await req('POST','/api/btv/users/999999999/verify-pin',{ pin:'0000' }); r.status===404 ? P(V,'verify-pin id inexistente','404 correto') : F(V,'verify-pin id inexistente',`esperava 404, veio ${r.status}`) })()
    await (async()=>{ const r = await req('POST','/api/btv/users/999999999/pin',{ pin:'' }); r.status===404 ? P(V,'set-pin id inexistente','404 correto') : F(V,'set-pin id inexistente',`esperava 404, veio ${r.status}`) })()
    await (async()=>{ const r = await req('POST','/api/btv/users/999999999/ativo',{ ativo:false }); r.status===404 ? P(V,'set-ativo id inexistente','404 correto') : W(V,'set-ativo id inexistente',`no-op silencioso: ${r.status} (não distingue perfil inexistente)`) })()
    // Fronteira de método: rotas POST-only não podem responder a GET
    await (async()=>{ const r = await req('GET','/api/ledger/verify'); r.status===405 ? P(V,'GET em rota POST-only (ledger/verify)','405 correto') : r.status===200 ? F(V,'GET em rota POST-only (ledger/verify)','200! método não checado') : W(V,'GET em rota POST-only (ledger/verify)',`veio ${r.status}`) })()
    await (async()=>{ const r = await req('GET','/api/verify/run'); r.status===405 ? P(V,'GET em rota POST-only (verify/run)','405 correto') : r.status===200 ? F(V,'GET em rota POST-only (verify/run)','200! disparou verify por GET') : W(V,'GET em rota POST-only (verify/run)',`veio ${r.status}`) })()
    await (async()=>{ const r = await req('GET','/api/rota-que-nao-existe-'+Math.floor(performance.now())); r.status===404 ? P(V,'rota inexistente','404 correto') : W(V,'rota inexistente',`veio ${r.status}`) })()
  }

  // ===================== 2) HONESTIDADE / DADOS FABRICADOS =====================
  if (CFG.api) {
    const H = 'HONESTO'
    // 2a. Ledger hash-chain: ok=false é integridade QUEBRADA — barulho alto.
    await (async()=>{
      const v = await req('POST','/api/ledger/verify'); const led = await req('GET','/api/ledger?limit=1000')
      const total = (led.data||[]).length
      if (v.data?.ok === true) P(H,'ledger hash-chain íntegro',`ok=true verified=${v.data?.verified} de ${total}`)
      else W(H,'ledger hash-chain NÃO íntegro',`ok=${v.data?.ok} verified=${v.data?.verified} de ${total} entradas — investigar volume/corrupção do .btv/btv.db`)
    })()
    // 2b. Custo NUNCA pode ser >0 com tokens zerados (seria fabricação). E o total tem de somar.
    await (async()=>{
      const r = await req('GET','/api/models/usage'); const e = r.data?.entries||[]
      const fab = e.filter(x => (x.estimated_cost_usd>0) && (x.input_tokens===0) && (x.output_tokens===0))
      if (fab.length) return F(H,'custo fabricado',`${fab.map(x=>x.model).join(', ')} têm custo>0 com 0 tokens`)
      const soma = e.reduce((a,x)=>a+(x.estimated_cost_usd||0),0)
      near(soma, r.data?.total_estimated_cost_usd||0, 1e-6)
        ? P(H,'custo honesto & total soma',`total=$${(r.data?.total_estimated_cost_usd||0).toFixed(6)} (${e.length} modelos, tokens→custo)`)
        : W(H,'total de custo não bate com a soma',`Σentries=${soma} ≠ total=${r.data?.total_estimated_cost_usd}`)
    })()
    // 2c. Provider usado tem de estar entre os configurados (fallback real, não fixo).
    await (async()=>{
      const pv = await req('GET','/api/providers'); const us = await req('GET','/api/models/usage')
      const conf = new Set((pv.data||[]).filter(p=>p.configured).map(p=>String(p.name||p.id||p.provider).toLowerCase()))
      const used = [...new Set((us.data?.entries||[]).map(x=>String(x.provider||'').toLowerCase()).filter(Boolean))]
      const orfaos = used.filter(u=>!conf.has(u))
      if (!used.length) I(H,'providers × uso','sem uso com provider rotulado ainda')
      else if (orfaos.length) W(H,'uso de provider não configurado',`usou [${orfaos.join(', ')}] fora dos configurados [${[...conf].join(', ')}]`)
      else P(H,'uso condiz com providers configurados',`usados [${used.join(', ')}] ⊆ configurados [${[...conf].join(', ')}]`)
    })()
    // 2d. summary.total_events vs /api/events (invariante ou paginação honesta)
    await (async()=>{
      const s = await req('GET','/api/summary'); const ev = await req('GET','/api/events')
      const te = s.data?.total_events, n = (ev.data||[]).length
      if (te===n) P(H,'summary.total_events == len(events)',`${te}`)
      else if (n < te) I(H,'events paginado','len(events)='+n+' < total_events='+te+' (janela recente — ok se documentado)')
      else W(H,'invariante de eventos',`total_events=${te} < len(events)=${n} (?)`)
    })()
    // 2e. Integridade de dados dos templates: papéis/formatos não podem vir vazios.
    await (async()=>{
      const r = await req('GET','/api/btv/templates'); const t = r.data||[]
      const req_fields = ['id','nome','cor','categoria','onda']
      const quebrados = t.filter(x => req_fields.some(f=>!x[f]) || !(x.papeis?.length) || !(x.formatos?.length))
      if (quebrados.length) F(H,'template com campo vazio',`${quebrados.map(x=>x.id||'?').join(', ')} sem papéis/formatos/campo obrigatório`)
      else P(H,'12 templates com dados completos',`${t.length} templates: papéis, formatos e metadados presentes`)
    })()
  }

  // ===================== 3) SAÚDE DA UI (erros de runtime / silêncios visuais) =====================
  if (CFG.ui && document.querySelector('#btv-root')) {
    const U = 'UI'
    const title = ()=> (document.querySelector('#btv-root h1.screen-title')?.textContent||'').trim()
    const buttons = ()=> [...document.querySelectorAll('#btv-root button')]
    const clickText = (txt)=>{ const b = buttons().find(x=>(x.textContent||'').includes(txt)); if(!b) throw new Error('sem botão '+txt); b.click(); return b }
    const waitUntil = async (fn,ms=CFG.uiWaitMs)=>{ const t0=Date.now(); for(;;){ let v; try{v=fn()}catch{v=false} if(v)return v; if(Date.now()-t0>ms) throw new Error('timeout UI'); await sleep(120) } }
    const goPersona = async (p)=>{ clickText(p==='admin'?'Administração':'Meu espaço'); await sleep(80) }
    const nav = async (label,exp)=>{ clickText(label); await waitUntil(()=>title()===exp) }
    const rootText = ()=> (document.querySelector('#btv-root')?.innerText||'')
    const SCREENS = [
      ['user','Início','Monte uma squad, receba entregas'],['user','Minhas squads','Minhas squads'],
      ['user','Personas','Personas & prompts'],['user','Biblioteca','Biblioteca de entregas'],
      ['user','Designer','Squad Designer'],['admin','Telemetria','Telemetria & custos'],
      ['admin','Ledger','Ledger de auditoria'],['admin','Providers','Providers & rate limits'],
      ['admin','Permissões','Permissões — skills, tools e MCP'],['admin','Modelos','Modelos de squad'],
      ['admin','Usuários','Usuários & acessos'],
    ]
    // Captura erros de runtime enquanto navega TODAS as telas.
    const errors = []
    const origErr = console.error, origWarn = console.warn
    const onErr = (e)=>errors.push('window.onerror: '+(e?.message||e))
    console.error = (...a)=>{ errors.push(a.map(String).join(' ').slice(0,200)); origErr.apply(console,a) }
    window.addEventListener('error', onErr)
    let visited = 0
    for (const [persona,label,expTitle] of SCREENS) {
      try {
        await goPersona(persona); await nav(label, expTitle); await sleep(250); visited++
        const txt = rootText()
        if (/carregando…/.test(txt)) W(U, `${label}: preso em carregando`, 'a tela não saiu de "carregando…"')
        if (/Não consegui/.test(txt)) W(U, `${label}: ErroBox visível`, txt.match(/Não consegui[^\n]{0,80}/)?.[0])
        const bad = txt.match(/(^|[\s(>])(NaN|undefined|\[object Object\])([\s.,)<]|$)/)
        if (bad) W(U, `${label}: valor cru no DOM`, `“${bad[2]}” — provável binding quebrado`)
      } catch (e) { F(U, `${label}: navegação`, e.message||String(e)) }
    }
    console.error = origErr; console.warn = origWarn; window.removeEventListener('error', onErr)
    errors.length ? F(U,'erros de runtime na navegação',`${errors.length}: ${errors.slice(0,3).join(' | ')}`) : P(U,'sem erros de runtime',`${visited}/${SCREENS.length} telas navegadas limpas`)
    // Placeholder honesto: rodapé fixo "Marina L." não é o usuário real (não há sessão logada).
    if (/Marina L\./.test(rootText())) {
      I(U,'perfil da sidebar é placeholder','“Marina L. / perfil usuário” é fixo — A6 tem perfis mas não há sessão/login real')
    }
  } else if (CFG.ui) {
    W('UI','SPA carregada','sem #btv-root — abra a RAIZ do dashboard (btv-web)')
  }

  // ===================== 4) VAZAMENTOS / RESÍDUOS =====================
  if (CFG.api) {
    const L = 'LEAK'
    // 4a. perfis de teste acumulados (smoke não tem rota de delete de usuário)
    await (async()=>{
      const r = await req('GET','/api/btv/users'); const test = (r.data||[]).filter(u=>/^(SMOKE|FULL)·pin·/.test(u.nome||''))
      test.length ? W(L,'perfis de teste acumulados',`${test.length} perfis “(SMOKE|FULL)·pin·…” residuais (sem rota de delete de usuário)`) : P(L,'sem perfis de teste residuais','0')
    })()
    // 4b. prompts create/delete voltam à linha de base (sem vazamento)
    await (async()=>{
      const base = (await req('GET','/api/prompts')).data?.length ?? 0
      const ids = []
      for (let i=0;i<3;i++){ const c = await req('POST','/api/prompts',{ name:'AUDIT '+i+'-'+Math.floor(performance.now()), generator:'audit', fields:{}, rendered:'x', tags:['audit'] }); ids.push(c.data?.id ?? c.data) }
      for (const id of ids) await req('DELETE',`/api/prompts/${id}`)
      const after = (await req('GET','/api/prompts')).data?.length ?? 0
      after===base ? P(L,'prompts create/delete sem vazamento',`voltou a ${base}`) : F(L,'vazamento de prompts',`base ${base} → depois ${after}`)
    })()
  }

  // ===================== 5) INTRUSIVO (opt-in): XSS armazenado =====================
  if (CFG.intrusive && CFG.api && document.querySelector('#btv-root')) {
    const S='SEC'
    await (async()=>{
      window.__xssAudit = undefined
      const nome = `<img src=x onerror="window.__xssAudit=1">·AUDIT·${Math.floor(performance.now())}`
      const c = await req('POST','/api/btv/users',{ nome, email:'', papel:'usuario' })
      const id = c.data?.id
      try {
        // renderiza a lista e espera
        const btn = (txt)=>[...document.querySelectorAll('#btv-root button')].find(b=>(b.textContent||'').includes(txt))
        btn('Administração')?.click(); await sleep(150)
        btn('Usuários')?.click(); await sleep(600)
        const executou = window.__xssAudit === 1
        const imgInjetada = !![...document.querySelectorAll('#btv-root img')].find(i=>i.getAttribute('src')==='x')
        if (executou || imgInjetada) F(S,'XSS armazenado',`payload EXECUTOU/injetou <img> (executou=${executou} img=${imgInjetada})`)
        else P(S,'nome é escapado (sem XSS)','React renderizou o payload como texto literal')
      } finally { if (id!=null) await req('POST',`/api/btv/users/${id}/ativo`,{ ativo:false }) }
      I(S,'resíduo do teste intrusivo',`perfil ${id} criado e suspenso (não há delete) — nome com payload inerte`)
    })()
  }

  // restaura UI
  try { const b=[...document.querySelectorAll('#btv-root button')]; (b.find(x=>(x.textContent||'').includes('Meu espaço'))||{click(){}}).click(); await sleep(60); const t=[...document.querySelectorAll('#btv-root button')].find(x=>(x.textContent||'').includes('Início')); t&&t.click() } catch {}

  const by = rows.reduce((a,r)=>{a[r.status]=(a[r.status]||0)+1;return a},{})
  console.log(`%c AUDITORIA: ${by.PASS||0} PASS · ${by.WARN||0} WARN · ${by.INFO||0} INFO · ${by.FAIL||0} FAIL`, 'font-weight:bold;font-size:13px')
  console.table(rows)
  if (by.FAIL) console.log('%c ❌ Achados que exigem correção — veja as linhas FAIL.', 'color:#c0392b;font-weight:bold')
  if (by.WARN) console.log('%c 🟡 Decisões silenciosas / suspeitas — veja as linhas WARN.', 'color:#b9770e;font-weight:bold')
  return rows
}
btvAudit()
