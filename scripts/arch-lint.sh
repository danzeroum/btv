#!/usr/bin/env bash
# Lint arquitetural (Trilha T4 do plano DDD multitenant).
#
# O CI é o revisor de arquitetura: este script falha o build quando uma
# fronteira de camada é violada, em vez de depender de convenção.
#
#   A) O crate de domínio (`btv-domain`, Trilha A) não pode depender de
#      infraestrutura — rusqlite/axum/tonic/reqwest — nem transitivamente
#      (arestas `normal`; dev-dependencies ficam livres: a suíte de contrato
#      pode usar o que precisar). Enquanto o crate não existe, a checagem é
#      pulada com AVISO explícito e arma sozinha quando ele nascer.
#   B) Arquivo de handler HTTP não pode conter SQL cru nem importar rusqlite —
#      persistência entra só por tipos de `btv-store` (hoje) ou pelos ports do
#      domínio (após a Trilha B). O padrão mira literais SQL e `use rusqlite`,
#      não a palavra "sqlite" (que aparece legitimamente em comentários e
#      nomes de teste).
#
#   E) A superfície axum da `btv-cli` está CONGELADA na allowlist do motor
#      (C4, fecho da campanha; ADR 0031). O guarda "btv-cli sem axum" armado
#      desde a semana 1 ativa aqui — mas com a fronteira VERDADEIRA que o C4
#      revelou: não é "axum vs CLI", é "console/dashboard vs motor". Um módulo
#      axum NOVO fora da allowlist → vermelho.
set -euo pipefail
cd "$(dirname "$0")/.."

falhas=0

# ── A: dependências do domínio ──────────────────────────────────────────────
if cargo metadata --format-version 1 --no-deps | grep -qE '"name" ?: ?"btv-domain"'; then
    proibidas=$(cargo tree -p btv-domain -e normal --prefix none \
        | awk '{print $1}' | sort -u \
        | grep -x -E 'rusqlite|axum|tonic|reqwest' || true)
    if [ -n "$proibidas" ]; then
        echo "ERRO(T4-A): btv-domain depende de infraestrutura proibida:"
        echo "$proibidas"
        falhas=1
    else
        echo "OK(T4-A): btv-domain livre de rusqlite/axum/tonic/reqwest (inclusive transitivos)."
    fi
else
    echo "AVISO(T4-A): crate btv-domain ainda não existe — checagem de dependência pulada."
    echo "             Ela arma automaticamente quando o crate nascer (Trilha A do plano DDD)."
fi

# ── C: btv-core só-portas (D1t) ─────────────────────────────────────────────
# O runtime de agente depende SÓ do domínio: nenhum concreto de LLM/storage/
# ferramentas (e portanto nem reqwest/rusqlite transitivos). Arestas
# `normal`; dev-dependencies livres (o fio com concretos é provado nos
# crates das implementações — btv-tools/btv-store — desde o D1t).
proibidas_core=$(cargo tree -p btv-core -e normal --prefix none \
    | awk '{print $1}' | sort -u \
    | grep -x -E 'btv-llm|btv-store|btv-tools|reqwest|rusqlite' || true)
if [ -n "$proibidas_core" ]; then
    echo "ERRO(T4-C): btv-core depende de concreto proibido (D1t exige só portas):"
    echo "$proibidas_core"
    falhas=1
else
    echo "OK(T4-C): btv-core depende só do domínio (sem btv-llm/btv-store/btv-tools)."
fi

# ── B: SQL cru em handlers HTTP ─────────────────────────────────────────────
# btv-server inteiro por glob (a C2 decompôs lib.rs em handlers/ — lista
# fixa deixaria os arquivos novos fora da varredura, lacuna real pega na
# própria C2); os módulos-roteadores de btv-cli seguem nominais até a C4.
mapfile -t alvos < <(find crates/btv-server/src -name '*.rs' | sort)
alvos+=(
    crates/btv-cli/src/btv_agent.rs
    crates/btv-cli/src/web_agent.rs
    crates/btv-cli/src/squad_agent.rs
)
padrao='rusqlite::|^use rusqlite|"(SELECT |INSERT INTO |DELETE FROM |UPDATE [A-Za-z_]+ SET |CREATE TABLE )'
if grep -nE "$padrao" "${alvos[@]}"; then
    echo "ERRO(T4-B): SQL cru / rusqlite em arquivo de handler HTTP (ocorrências acima)."
    echo "            Persistência entra por btv-store (hoje) ou pelos ports do domínio (Trilha B)."
    falhas=1
else
    echo "OK(T4-B): handlers HTTP sem SQL cru e sem rusqlite."
fi

# ── D: BTV_MODE só no extractor (E1s.2) ─────────────────────────────────────
# A resolução por modo (local×saas) é PEÇA ÚNICA: vive SÓ no extractor de
# tenant. "Seis ifs de BTV_MODE espalhados são o SQL-em-handler da
# autenticação" — a mesma doença (regra transversal por cópia), a mesma cura
# (peça única + juiz mecânico). Qualquer leitura de `BTV_MODE` em código Rust
# fora do módulo do extractor é a regra vazando de camada. Mesmo mecanismo
# grep-por-fronteira do T4-B.
extrator='crates/btv-cli/src/tenant_extractor.rs'
fora_do_extrator=$(grep -rln 'BTV_MODE' crates --include='*.rs' | grep -vx "$extrator" || true)
if [ -n "$fora_do_extrator" ]; then
    echo "ERRO(T4-D): BTV_MODE lido fora do extractor de tenant ($extrator):"
    echo "$fora_do_extrator"
    echo "            A resolução por modo é peça única (E1s.2) — nenhum handler decide modo."
    falhas=1
else
    echo "OK(T4-D): BTV_MODE só no extractor de tenant (resolução por modo é peça única)."
fi

# ── E: superfície axum da btv-cli CONGELADA na allowlist do motor (C4, fecho) ─
# O guarda armado desde a semana 1 ("btv-cli não importa axum") ativa AQUI —
# mas com a fronteira VERDADEIRA que o C4 revelou (ADR 0031): não é "axum vs
# CLI" (o levantamento de julho mediu a doença errada), é "console/dashboard vs
# MOTOR". Os consoles-folha (sandbox/doctor/lsp) consolidaram em btv-server; o
# que resta com axum na btv-cli é o MOTOR do produto (agent-loop/squad/borda) e
# os consoles que o SERVEM (permission-overlay do mcp, sidecars Python do
# memory/prompt_render) — eles servem HTTP porque o produto local É um servidor:
# o axum é a interface do motor, não vazamento de camada. A allowlist CONGELA
# essa superfície: um módulo axum NOVO fora dela é o console que devia nascer em
# btv-server nascendo no lar errado — o bug que a onda inteira combateu. Mesmo
# mecanismo grep-por-fronteira do T4-B/D.
allowlist_axum=(
    btv_agent btv_agent_golden mcp_console memory_console prompt_render
    squad_agent tenant_border_sweep tenant_extractor web_agent
)
fora_allowlist=""
while IFS= read -r arquivo; do
    base=$(basename "$arquivo" .rs)
    permitido=0
    for nome in "${allowlist_axum[@]}"; do
        [ "$base" = "$nome" ] && { permitido=1; break; }
    done
    [ "$permitido" = 0 ] && fora_allowlist="$fora_allowlist$arquivo"$'\n'
done < <(grep -rlE '^use axum|axum::' crates/btv-cli/src --include='*.rs' | sort)
if [ -n "$fora_allowlist" ]; then
    echo "ERRO(T4-E): módulo axum NOVO na btv-cli fora da allowlist do motor:"
    printf '%s' "$fora_allowlist"
    echo "            Console/dashboard novo nasce em btv-server (ADR 0031); a btv-cli só"
    echo "            serve HTTP pelo MOTOR (agent-loop/squad/borda + consoles que o servem)."
    falhas=1
else
    echo "OK(T4-E): superfície axum da btv-cli congelada na allowlist do motor (ADR 0031)."
fi

exit "$falhas"
