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
# Checagem futura já prevista no plano (NÃO ativa — seria falso-positivo
# hoje): `btv-cli` sem axum, só depois de C4 (os 9 módulos-roteadores migram
# para btv-server).
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

# ── B: SQL cru em handlers HTTP ─────────────────────────────────────────────
alvos=(
    crates/btv-server/src/lib.rs
    crates/btv-server/src/btv.rs
    crates/btv-server/src/bin/loadgen.rs
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

exit "$falhas"
