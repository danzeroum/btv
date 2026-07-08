import type { ScreenId } from '../types/domain'

export interface ScreenMeta {
  kicker: string
  title: string
  note: string
  /** Cor do kicker. `vivo` usa a cor da squad ativa em runtime (Shell resolve). */
  accent: string
}

/** Cabeçalhos por tela — texto verbatim do protótipo (`heads` em renderVals). */
export const SCREEN_META: Record<ScreenId, ScreenMeta> = {
  inicio: {
    kicker: 'perfil usuário · U1',
    title: 'Monte uma squad, receba entregas',
    note: 'Cada modelo é uma equipe pronta: papéis, esteira, gates de aprovação e formatos de exportação da sua área.',
    accent: 'var(--brand)',
  },
  vivo: {
    kicker: 'perfil usuário · U3',
    title: 'Squad ao vivo',
    note: 'A esteira mostra onde o trabalho está. Quando um gate abre, a squad espera por você.',
    accent: 'var(--brand)',
  },
  biblioteca: {
    kicker: 'perfil usuário · U4',
    title: 'Biblioteca de entregas',
    note: 'Todo artefato carrega sua procedência: quem produziu, quem revisou, qual gate aprovou.',
    accent: 'var(--brand)',
  },
  designer: {
    kicker: 'perfil usuário · U5',
    title: 'Squad Designer',
    note: 'A posição horizontal define a ordem da esteira. Selecione um bloco para editar nome e prompt, teste a squad e salve como modelo.',
    accent: 'var(--brand)',
  },
  minhas: {
    kicker: 'perfil usuário · U6',
    title: 'Minhas squads',
    note: 'Tudo que está rodando ou já rodou — retome, reative ou vá direto às entregas.',
    accent: 'var(--brand)',
  },
  personas: {
    kicker: 'perfil usuário · U7',
    title: 'Personas & prompts',
    note: 'A equipe de cada modelo é sua: edite o prompt de qualquer papel, crie personas novas ou volte ao padrão.',
    accent: 'var(--brand)',
  },
  telemetria: {
    kicker: 'administração · A1',
    title: 'Telemetria & custos',
    note: 'Visão do parque de squads para decidir limites e prioridades.',
    accent: 'var(--muted)',
  },
  ledger: {
    kicker: 'administração · A2',
    title: 'Ledger de auditoria',
    note: 'Trilha imutável: ativações, gates, exportações e chamadas de ferramenta.',
    accent: 'var(--muted)',
  },
  providers: {
    kicker: 'administração · A3',
    title: 'Providers & rate limits',
    note: 'Provedores de modelo conectados, consumo de limite e fallback.',
    accent: 'var(--muted)',
  },
  permissoes: {
    kicker: 'administração · A4',
    title: 'Permissões — skills, tools e MCP',
    note: 'O que cada template de squad pode usar. Negar aqui vale imediatamente.',
    accent: 'var(--muted)',
  },
  modelos: {
    kicker: 'administração · A5',
    title: 'Modelos de squad',
    note: 'Publique, versione e promova templates — inclusive os criados no Designer.',
    accent: 'var(--muted)',
  },
  usuarios: {
    kicker: 'administração · A6',
    title: 'Usuários & acessos',
    note: 'Quem usa o BuildToValue, com que papel, e o que pode ativar.',
    accent: 'var(--muted)',
  },
}
