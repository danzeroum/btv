import { useCallback, useEffect, useRef, useState } from 'react'
import { createUser, fetchUsers, setUserAtivo, type BtvUser } from '../../../api/admin'
import { ErroBox, NotaHonesta, Pill, Toggle } from './comum'

// Avatares em família grafite/taupe — iniciais discretas, sem cores funcionais
// (regra §5 Usuários: "avatar de iniciais grafite").
const CORES = ['#2b2b28', '#4a463f', '#6f675a', '#5b5344', '#8b8171', '#3c3934']

/** A6 · Usuários & acessos — perfis LOCAIS persistidos (BtvStore), sem
 *  senha/autenticação (local-first, 127.0.0.1): identidade nomeada para
 *  atribuição. Auth real é trabalho futuro explícito. */
export function Usuarios() {
  const [users, setUsers] = useState<BtvUser[] | null>(null)
  const [erro, setErro] = useState<string | null>(null)
  const nomeRef = useRef<HTMLInputElement | null>(null)
  const emailRef = useRef<HTMLInputElement | null>(null)

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
    void createUser(nome, email, users.length === 0 ? 'admin' : 'usuario').then(() => {
      nomeRef.current!.value = ''
      if (emailRef.current) emailRef.current.value = ''
      recarregar()
    })
  }

  return (
    <>
      <div style={{ display: 'flex', gap: 8 }}>
        <input ref={nomeRef} placeholder="nome do perfil…" style={{ flex: 1, border: '1px solid var(--line2)', borderRadius: 10, padding: '10px 14px', fontSize: 13, background: 'var(--white)', color: 'var(--ink)' }} />
        <input ref={emailRef} placeholder="e-mail (opcional)" style={{ flex: 1, border: '1px solid var(--line2)', borderRadius: 10, padding: '10px 14px', fontSize: 13, background: 'var(--white)', color: 'var(--ink)' }} />
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
          <div key={u.id} data-testid={`user-${u.id}`} style={{ display: 'grid', gridTemplateColumns: 'auto 1.6fr 120px auto', gap: 18, alignItems: 'center', background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 12, padding: '14px 20px' }}>
            <span style={{ width: 34, height: 34, borderRadius: '50%', background: CORES[i % CORES.length], color: '#fff', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 13 }}>
              {u.nome[0]?.toUpperCase() ?? '?'}
            </span>
            <span style={{ display: 'flex', flexDirection: 'column', gap: 1, minWidth: 0 }}>
              <span style={{ fontSize: 13.5, fontWeight: 600 }}>{u.nome}</span>
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
          </div>
        ))}
      </div>
      <NotaHonesta>
        Perfis locais sem senha (127.0.0.1): servem para atribuição nomeada — autenticação real é
        trabalho futuro explícito, não uma tela fingindo login.
      </NotaHonesta>
    </>
  )
}
