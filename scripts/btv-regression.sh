#!/usr/bin/env bash
# ============ BTV — regressão dos consertos via curl (sem frontend) ============
# Prova, no backend real, cada correção desta rodada. Complementa os testes
# Rust (cargo test) e o vitest — aqui é o contrato HTTP ponta-a-ponta na
# instância que estiver rodando. NÃO deixa resíduo (o perfil de teste é criado
# e removido).
#
# Uso:
#   BASE=https://squad.buildtovalue.cloud AUTH=squad:squad123 bash scripts/btv-regression.sh
#   BASE=http://127.0.0.1:7878 bash scripts/btv-regression.sh
set -u
BASE="${BASE:-http://127.0.0.1:7878}"
AUTH="${AUTH:-}"
AUTH_ARGS=(); [ -n "$AUTH" ] && AUTH_ARGS=(-u "$AUTH")
have_jq=0; command -v jq >/dev/null 2>&1 && have_jq=1

pass=0; fail=0
P(){ printf '✅ %s%s\n' "$1" "${2:+ — $2}"; pass=$((pass+1)); }
Fl(){ printf '❌ %s%s\n' "$1" "${2:+ — $2}"; fail=$((fail+1)); }
code(){ local m="$1" p="$2"; shift 2; curl -sk -o /dev/null -w '%{http_code}' -X "$m" "${AUTH_ARGS[@]}" "$@" "$BASE$p"; }
body(){ local m="$1" p="$2"; shift 2; curl -sk -X "$m" "${AUTH_ARGS[@]}" "$@" "$BASE$p"; }
# extrai .id de um JSON simples ({"id":N}) com jq ou sed
json_id(){ if [ "$have_jq" = 1 ]; then jq -r '.id // empty'; else sed -nE 's/.*"id":[[:space:]]*([0-9]+).*/\1/p'; fi; }

echo "=== BTV regressão @ $BASE (auth=${AUTH:-nenhum}) ==="

# 1) fix(set-ativo): id inexistente → 404 (antes: 200 no-op silencioso)
c=$(code POST /api/btv/users/999999999/ativo -H 'content-type: application/json' -d '{"ativo":false}')
[ "$c" = 404 ] && P "set-ativo id inexistente → 404" || Fl "set-ativo id inexistente" "veio $c (esperado 404)"

# 2) fix(rota /api): rota /api desconhecida → 404 JSON (antes: 200 HTML do SPA)
c=$(code GET "/api/rota-inexistente-regressao")
ct=$(curl -sk -o /dev/null -w '%{content_type}' "${AUTH_ARGS[@]}" "$BASE/api/rota-inexistente-regressao")
if [ "$c" = 404 ]; then
  case "$ct" in *json*) P "/api desconhecida → 404 JSON" "$ct";; *) Fl "/api desconhecida" "404 mas content-type=$ct (esperava json)";; esac
else Fl "/api desconhecida" "veio $c (esperado 404; 200 = ainda caindo no SPA)"; fi

# 3) feat(delete user): create → delete → 404 no re-delete → sumiu da lista (self-clean)
novo=$(body POST /api/btv/users -H 'content-type: application/json' -d '{"nome":"REGRESSAO·tmp","email":"","papel":"usuario"}')
id=$(printf '%s' "$novo" | json_id)
if [ -n "$id" ]; then
  d1=$(code DELETE "/api/btv/users/$id")
  d2=$(code DELETE "/api/btv/users/$id")
  aindaExiste=$(body GET /api/btv/users | grep -c "\"id\":$id" || true)
  if [ "$d1" = 200 ] && [ "$d2" = 404 ] && [ "${aindaExiste:-0}" = 0 ]; then
    P "delete user: 200, re-delete 404, removido da lista" "id $id"
  else
    Fl "delete user" "delete=$d1 re-delete=$d2 aindaNaLista=$aindaExiste (esperado 200/404/0)"
  fi
else
  Fl "delete user" "não consegui criar o perfil de teste (resp: $(printf '%s' "$novo" | head -c 80))"
fi

# 4) fix(squad ao vivo — contrato de backend): gate/ajuste em task inexistente → 404
#    (é a falha que o frontend agora TRATA em vez de virar erro não-tratado)
c=$(code POST /api/btv/squads/task-inexistente-regressao/gate -H 'content-type: application/json' -d '{}')
[ "$c" = 404 ] && P "aprovar gate em task inexistente → 404" || Fl "aprovar gate task inexistente" "veio $c"
c=$(code POST /api/btv/squads/task-inexistente-regressao/ajuste -H 'content-type: application/json' -d '{"instrucao":"x"}')
[ "$c" = 404 ] && P "pedir ajuste em task inexistente → 404" || Fl "pedir ajuste task inexistente" "veio $c"

# 5) aviso "sem artefato real": a fonte da verdade é a contagem de entregas por
#    run (arquivo REAL gravado por ferramenta). O endpoint responde uma lista.
c=$(code GET /api/btv/deliverables)
if [ "$c" = 200 ]; then
  if [ "$have_jq" = 1 ]; then
    tipo=$(body GET /api/btv/deliverables | jq -r 'if type=="array" then "array" else type end')
    [ "$tipo" = array ] && P "fonte do aviso: /api/btv/deliverables é lista" "$(body GET /api/btv/deliverables | jq 'length') entrega(s)" || Fl "deliverables" "não é array ($tipo)"
  else
    P "/api/btv/deliverables responde 200" "(sem jq: forma não checada)"
  fi
else Fl "/api/btv/deliverables" "veio $c"; fi

echo "======================================================================"
printf 'REGRESSÃO: %d PASS · %d FAIL\n' "$pass" "$fail"
[ "$fail" -gt 0 ] && { echo "❌ Há regressões."; exit 1; } || { echo "✅ Todos os consertos verificados no backend."; exit 0; }
