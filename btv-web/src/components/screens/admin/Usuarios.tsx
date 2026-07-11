import { useCallback, useEffect, useRef, useState } from 'react'
import { createUser, deleteUser, fetchUsers, setUserAtivo, verifyUserPin, type BtvUser } from '../../../api/admin'
import { ErroBox, NotaHonesta, Pill, Toggle } from './comum'

// Avatares em família grafite/taupe — iniciais discretas, sem cores funcionais
// (regra §5 Usuários: "avatar de iniciais grafite").
const CORES = ['#2b2b28', '#4a463f', '#6f675a', '#5b5344', '#8b8171', '#3c3934']

/** A6 · Usuários & acessos — perfis LOCAIS persistidos (BtvStore) com PIN
 *  OPCIONAL verificado pelo backend (hash sha256, nunca em claro). O PIN
 *  gate o "assumir perfil"; não é uma barreira de rede (127.0.0.1, guardado
 *  por Origin). */
export function Usuarios() {
  const [users, setUsers] = useState<BtvUser[] | null>(null)
  const [erro, setErro] = useState<string | null>(null)
  const [ativo, setAtivo] = useState<{ id: number; nome: string } | null>(null)
  const [desafio, setDesafio] = useState<number | null>(null)
  const [pinErro, setPinErro] = useState<string | null>(null)
  const nomeRef = useRef<HTMLInputElement | null>(null)
  const emailRef = useRef<HTMLInputElement | null>(null)
  const pinRef = useRef<HTMLInputElement | null>(null)
  const desafioRef = useRef<HTMLInputElement | null>(null)

  const recarregar = useCallback(() => {
    fetchUsers()
      .then(setUsers)
      .catch((e: Error) => setErro(e.message))
  }, [])
  useEffect(() => recarregar(), [recarregar])

  if (erro) return <ErroBox msg={`Não consegui carregar os perfis (${erro}).`} />
  if (!users) return <div className="mono" style={{ fontSize: 11.5, color: 'var(--faint)' }}>carregando…</div>

  const adicionar = () => {
    const nome = nomeRef.current?.value.trim()
    if (!nome) return
    const email = emailRef.current?.value.trim() ?? ''
    const pin = pinRef.current?.value.trim() || undefined
    void createUser(nome, email, users.length === 0 ? 'admin' : 'usuario', pin)
      .then(() => {
        nomeRef.current!.value = ''
        if (emailRef.current) emailRef.current.value = ''
        if (pinRef.current) pinRef.current.value = ''
        recarregar()
      })
      .catch((e: Error) => setErro(e.message))
  }

  const entrar = (u: BtvUser) => {
    setPinErro(null)
    if (!u.has_pin) {
      // Perfil aberto — assume direto (nada a verificar).
      setAtivo({ id: u.id, nome: u.nome })
      return
    }
    setDesafio(u.id)
  }

  const confirmarPin = (u: BtvUser) => {
    const pin = desafioRef.current?.value ?? ''
    void verifyUserPin(u.id, pin)
      .then((r) => {
      if (r.ok) {
        setAtivo({ id: u.id, nome: u.nome })
        setDesafio(null)
        setPinErro(null)
      } else {
        setPinErro('PIN incorreto.')
      }
      })
      .catch(() => setPinErro('Não consegui verificar o PIN. Tente de novo.'))
  }

  const remover = (u: BtvUser) => {
    if (!window.confirm(`Remover o perfil "${u.nome}"? Esta ação não pode ser desfeita.`)) return
    void deleteUser(u.id)
      .then(() => {
        if (ativo?.id === u.id) setAtivo(null)
        recarregar()
      })
      .catch((e: Error) => setErro(e.message))
  }

  return (
    <>
      {ativo && (
        <div style={{ background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 12, padding: '12px 18px', display: 'flex', alignItems: 'center', gap: 10, fontSize: 13 }}>
          <span>Perfil ativo: <strong>{ativo.nome}</strong></span>
          <button onClick={() => setAtivo(null)} style={{ marginLeft: 'auto', background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '5px 12px', fontSize: 11.5, color: 'var(--muted)', fontFamily: 'var(--mono)' }}>
            sair
          </button>
        </div>
      )}

      <div style={{ display: 'flex', gap: 8 }}>
        <input ref={nomeRef} placeholder="nome do perfil…" style={{ flex: 1, border: '1px solid var(--line2)', borderRadius: 10, padding: '10px 14px', fontSize: 13, background: 'var(--white)', color: 'var(--ink)' }} />
        <input ref={emailRef} placeholder="e-mail (opcional)" style={{ flex: 1, border: '1px solid var(--line2)', borderRadius: 10, padding: '10px 14px', fontSize: 13, background: 'var(--white)', color: 'var(--ink)' }} />
        <input ref={pinRef} type="password" placeholder="PIN (opcional)" style={{ width: 130, border: '1px solid var(--line2)', borderRadius: 10, padding: '10px 14px', fontSize: 13, background: 'var(--white)', color: 'var(--ink)' }} />
        <button onClick={adicionar} style={{ background: 'var(--brand)', color: '#fff', border: 'none', borderRadius: 10, padding: '0 18px', fontSize: 12.5, fontWeight: 600, fontFamily: 'var(--sans)' }}>
          + Adicionar perfil
        </button>
      </div>

      {users.length === 0 && (
        <div style={{ background: 'var(--white)', border: '1px dashed var(--line2)', borderRadius: 14, padding: '24px 28px', color: 'var(--muted)', fontSize: 13.5 }}>
          Nenhum perfil ainda — o primeiro criado entra como admin.
        </div>
      )}

      <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        {users.map((u, i) => (
          <div key={u.id} data-testid={`user-${u.id}`} style={{ display: 'grid', gridTemplateColumns: 'auto 1.6fr 120px auto auto auto', gap: 16, alignItems: 'center', background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 12, padding: '14px 20px' }}>
            <span style={{ width: 34, height: 34, borderRadius: '50%', background: CORES[i % CORES.length], color: '#fff', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 13 }}>
              {u.nome[0]?.toUpperCase() ?? '?'}
            </span>
            <span style={{ display: 'flex', flexDirection: 'column', gap: 1, minWidth: 0 }}>
              <span style={{ fontSize: 13.5, fontWeight: 600, display: 'flex', alignItems: 'center', gap: 6 }}>
                {u.nome}
                {u.has_pin && (
                  <span title="perfil protegido por PIN" data-testid={`lock-${u.id}`} style={{ fontSize: 11 }}>
                    🔒
                  </span>
                )}
              </span>
              <span className="mono" style={{ fontSize: 10, color: 'var(--faint)' }}>{u.email || '—'}</span>
            </span>
            <Pill tone={u.papel === 'admin' ? 'ok' : 'muted'}>{u.papel}</Pill>
            <div style={{ display: 'flex', alignItems: 'center', gap: 9 }}>
              <span className="mono" style={{ fontSize: 10, color: 'var(--faint)' }}>
                {u.ativo ? 'ativo' : 'suspenso'}
              </span>
              <Toggle
                on={u.ativo}
                label={`acesso de ${u.nome}`}
                onClick={() => void setUserAtivo(u.id, !u.ativo).then(recarregar)}
              />
            </div>
            <button
              disabled={!u.ativo}
              data-testid={`entrar-${u.id}`}
              onClick={() => entrar(u)}
              style={{ background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '6px 13px', fontSize: 11.5, color: u.ativo ? 'var(--ink)' : 'var(--faint)', fontFamily: 'var(--mono)' }}
            >
              entrar
            </button>
            <button
              data-testid={`remover-${u.id}`}
              onClick={() => remover(u)}
              title="remover perfil"
              aria-label={`remover ${u.nome}`}
              style={{ background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '6px 11px', fontSize: 11.5, color: 'var(--muted)', fontFamily: 'var(--mono)' }}
            >
              remover
            </button>
            {desafio === u.id && (
              <div style={{ gridColumn: '1 / -1', display: 'flex', alignItems: 'center', gap: 8, marginTop: 4 }}>
                <input
                  ref={desafioRef}
                  type="password"
                  autoFocus
                  placeholder={`PIN de ${u.nome}`}
                  data-testid={`pin-input-${u.id}`}
                  onKeyDown={(e) => e.key === 'Enter' && confirmarPin(u)}
                  style={{ flex: 1, border: '1px solid var(--line2)', borderRadius: 8, padding: '8px 12px', fontSize: 12.5, background: 'var(--white)', color: 'var(--ink)' }}
                />
                <button onClick={() => confirmarPin(u)} style={{ background: 'var(--brand)', color: '#fff', border: 'none', borderRadius: 8, padding: '0 14px', fontSize: 12, fontWeight: 600, fontFamily: 'var(--sans)' }}>
                  confirmar
                </button>
                <button onClick={() => { setDesafio(null); setPinErro(null) }} style={{ background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '6px 12px', fontSize: 11.5, color: 'var(--muted)', fontFamily: 'var(--mono)' }}>
                  cancelar
                </button>
                {pinErro && <span style={{ color: 'var(--muted)', fontWeight: 600, fontSize: 11.5 }} data-testid={`pin-erro-${u.id}`}>{pinErro}</span>}
              </div>
            )}
          </div>
        ))}
      </div>
      <NotaHonesta>
        Perfis locais com PIN <strong>opcional</strong>: quem tem PIN é verificado pelo backend (hash
        sha256, nunca em claro) para ser assumido. Não é uma barreira de rede — o dashboard roda em
        127.0.0.1 sob a guarda de Origin; o PIN protege a atribuição do perfil, não uma sessão HTTP.
      </NotaHonesta>
    </>
  )
}
