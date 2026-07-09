#!/usr/bin/env bash
# ============ BTV — sondas adversariais via curl (rode na VPS) ============
# Prova o que o navegador NÃO consegue: guarda de Origin com headers forjados,
# fronteiras de método, payload malformado, fallback de SPA e invariantes de
# honestidade. Não muta dados de forma destrutiva (só POSTs que a app rejeita
# ou operações idempotentes).
#
# Uso:
#   BASE=https://squad.buildtovalue.cloud AUTH=squad:squad123 bash scripts/btv-adversarial.sh
#   # local, sem auth de borda:
#   BASE=http://127.0.0.1:7878 bash scripts/btv-adversarial.sh
#
# Variáveis:
#   BASE  (default http://127.0.0.1:7878)   URL raiz do dashboard
#   AUTH  (opcional)  user:pass do basic-auth da borda (nginx). Vazio = sem auth.
#   TRUSTED_ORIGIN (default = host do BASE)  origin que a guarda deve ACEITAR.
set -u

BASE="${BASE:-http://127.0.0.1:7878}"
AUTH="${AUTH:-}"
host_of() { printf '%s' "$1" | sed -E 's#^https?://##; s#/.*$##'; }
TRUSTED_ORIGIN="${TRUSTED_ORIGIN:-$(printf '%s' "$BASE" | sed -E 's#(https?://[^/]+).*#\1#')}"

AUTH_ARGS=(); [ -n "$AUTH" ] && AUTH_ARGS=(-u "$AUTH")
have_jq=0; command -v jq >/dev/null 2>&1 && have_jq=1

pass=0; warn=0; fail=0; info=0
P(){ printf '✅ [%s] %s%s\n' "$1" "$2" "${3:+ — $3}"; pass=$((pass+1)); }
W(){ printf '🟡 [%s] %s%s\n' "$1" "$2" "${3:+ — $3}"; warn=$((warn+1)); }
Fl(){ printf '❌ [%s] %s%s\n' "$1" "$2" "${3:+ — $3}"; fail=$((fail+1)); }
Inf(){ printf 'ℹ️  [%s] %s%s\n' "$1" "$2" "${3:+ — $3}"; info=$((info+1)); }

# code METHOD PATH [extra curl args...]  -> imprime só o status HTTP
code(){ local m="$1" p="$2"; shift 2; curl -sk -o /dev/null -w '%{http_code}' -X "$m" "${AUTH_ARGS[@]}" "$@" "$BASE$p"; }
# body METHOD PATH [extra...] -> imprime o corpo
body(){ local m="$1" p="$2"; shift 2; curl -sk -X "$m" "${AUTH_ARGS[@]}" "$@" "$BASE$p"; }
ctype(){ local p="$1"; shift; curl -sk -o /dev/null -w '%{content_type}' "${AUTH_ARGS[@]}" "$@" "$BASE$p"; }

echo "=== BTV adversarial @ $BASE (auth=${AUTH:-nenhum}, trusted-origin=$TRUSTED_ORIGIN) ==="
[ "$have_jq" = 1 ] || echo "(jq ausente — checagens de honestidade viram observação textual)"

# ---------------------------------------------------------------------------
echo "--- 1) Guarda de Origin (CSRF / ADR 0015) ---"
# 1a. mutação com Origin FORJADA deve receber 403 forbidden_origin
c=$(code POST /api/ledger/verify -H "Origin: https://evil.example")
if [ "$c" = 403 ]; then P GUARD "mutação com Origin forjada" "403 (bloqueado)"
elif [ "$c" = 401 ]; then W GUARD "mutação com Origin forjada" "401 — parou no basic-auth da borda antes da guarda; teste por dentro (BASE=127.0.0.1)"
else Fl GUARD "mutação com Origin forjada" "esperava 403, veio $c — guarda não efetiva (nginx removeu Origin? CSRF reaberto)"; fi
# 1b. mutação com Origin CONFIÁVEL não pode ser barrada pela guarda
c=$(code POST /api/ledger/verify -H "Origin: $TRUSTED_ORIGIN")
[ "$c" != 403 ] && P GUARD "mutação com Origin confiável" "passou a guarda ($c)" || Fl GUARD "mutação com Origin confiável" "403 — allowlist não contém $TRUSTED_ORIGIN (browser mutations quebrariam)"
# 1c. mutação SEM Origin (curl/CLI) é permitida por design
c=$(code POST /api/ledger/verify)
[ "$c" != 403 ] && P GUARD "mutação sem Origin (CLI)" "permitido ($c)" || Fl GUARD "mutação sem Origin (CLI)" "403 — quebraria a CLI/curl"
# 1d. GET com Origin forjada NÃO é barrado (guarda só olha não-GET)
c=$(code GET /api/summary -H "Origin: https://evil.example")
[ "$c" = 200 ] && P GUARD "GET com Origin forjada" "200 (guarda ignora GET, correto)" || W GUARD "GET com Origin forjada" "veio $c"

# ---------------------------------------------------------------------------
echo "--- 2) Fronteira de método / rota ---"
c=$(code GET /api/ledger/verify)
if   [ "$c" = 405 ]; then P METODO "GET em POST-only (ledger/verify)" "405"
elif [ "$c" = 200 ]; then Fl METODO "GET em POST-only (ledger/verify)" "200 — método não checado"
else W METODO "GET em POST-only (ledger/verify)" "veio $c"; fi
c=$(code GET /api/verify/run)
if   [ "$c" = 405 ]; then P METODO "GET em POST-only (verify/run)" "405"
elif [ "$c" = 200 ]; then Fl METODO "GET em POST-only (verify/run)" "200 — disparou verify por GET"
else W METODO "GET em POST-only (verify/run)" "veio $c"; fi
c=$(code GET "/api/rota-inexistente-$RANDOM")
[ "$c" = 404 ] && P METODO "rota inexistente" "404" || W METODO "rota inexistente" "veio $c — /api desconhecida cai no fallback SPA (index.html), não 404 JSON (por design)"

# ---------------------------------------------------------------------------
echo "--- 3) Input hostil (4xx, nunca 500) ---"
chk_input(){ # nome METHOD PATH  (payload via -d nos args seguintes)
  local nm="$1" m="$2" p="$3"; shift 3
  local c; c=$(code "$m" "$p" -H 'content-type: application/json' "$@")
  if   [ "$c" = 500 ]; then Fl INPUT "$nm" "500 em input inválido (bug)"
  elif [ "$c" -ge 400 ] && [ "$c" -lt 500 ]; then P INPUT "$nm" "rejeitado ($c)"
  else W INPUT "$nm" "aceitou input inválido ($c)"; fi
}
chk_input "user nome vazio"        POST /api/btv/users            -d '{"nome":"  ","email":""}'
chk_input "user corpo {}"          POST /api/btv/users            -d '{}'
chk_input "designer nodes=array"   POST /api/btv/designer/flows   -d '{"nome":"x","diagram":{"nodes":[],"edges":{}}}'
chk_input "verify-pin sem pin"     POST /api/btv/users/1/verify-pin -d '{}'
chk_input "JSON malformado"        POST /api/btv/users            -d '{ not json '
chk_input "verify-pin id inexistente" POST /api/btv/users/999999999/verify-pin -d '{"pin":"0000"}'
# set-ativo em id inexistente: no-op silencioso conhecido (não distingue 404)
c=$(code POST /api/btv/users/999999999/ativo -H 'content-type: application/json' -d '{"ativo":false}')
[ "$c" = 404 ] && P INPUT "set-ativo id inexistente" "404" || W INPUT "set-ativo id inexistente" "no-op silencioso ($c)"

# ---------------------------------------------------------------------------
echo "--- 4) Fallback de SPA / assets ---"
ct=$(ctype "/rota-de-spa-inexistente-$RANDOM")
case "$ct" in *text/html*) P SPA "deep-link cai no index.html" "$ct";; *) W SPA "deep-link" "content-type=$ct (esperava text/html)";; esac
c=$(code GET /dev/)
[ "$c" = 200 ] && P SPA "console dev em /dev" "200" || W SPA "console dev em /dev" "veio $c"

# ---------------------------------------------------------------------------
echo "--- 5) Honestidade / dados fabricados ---"
if [ "$have_jq" = 1 ]; then
  # 5a. ledger hash-chain
  led=$(body POST /api/ledger/verify); ok=$(printf '%s' "$led" | jq -r '.ok'); ver=$(printf '%s' "$led" | jq -r '.verified')
  tot=$(body GET '/api/ledger?limit=1000' | jq 'length' 2>/dev/null)
  if [ "$ok" = true ]; then P HONESTO "ledger hash-chain íntegro" "ok=true verified=$ver de $tot"
  else W HONESTO "ledger hash-chain NÃO íntegro" "ok=$ok verified=$ver de $tot — investigar .btv/btv.db (volume?)"; fi
  # 5b. shape de models/usage — array puro denuncia backend PRÉ-MERGE
  usg=$(body GET /api/models/usage)
  shape=$(printf '%s' "$usg" | jq -r 'if (type=="object" and has("entries")) then "novo" elif type=="array" then "antigo" else "?" end' 2>/dev/null)
  [ "$shape" = "antigo" ] && W HONESTO "models/usage é ARRAY puro" "backend PRÉ-MERGE (sem entries/pricing_as_of) — provável instância ANTIGA neste \$BASE"
  ent='(.entries // .)'   # tolera as duas formas
  fab=$(printf '%s' "$usg" | jq "[ ${ent}[] | select(.estimated_cost_usd>0 and .input_tokens==0 and .output_tokens==0)] | length" 2>/dev/null)
  as_of=$(printf '%s' "$usg" | jq -r '.pricing_as_of // "n/d"' 2>/dev/null)
  if [ "${fab:-0}" -gt 0 ]; then Fl HONESTO "custo fabricado" "$fab modelo(s) com custo>0 e 0 tokens"
  else P HONESTO "custo honesto" "sem custo fabricado (tab $as_of)"; fi
  # 5c. provider usado ⊆ configurados
  conf=$(body GET /api/providers | jq -r '[.[]|select(.configured)|(.name//.id//.provider)]|@csv' 2>/dev/null)
  used=$(printf '%s' "$usg" | jq -r "[ ${ent}[].provider ]|unique|@csv" 2>/dev/null)
  Inf HONESTO "providers configurados × usados" "conf=[$conf] usados=[$used]"
  # 5d. templates com dados completos
  bad=$(body GET /api/btv/templates | jq '[.[] | select((.papeis|length)==0 or (.formatos|length)==0 or (.id|not) or (.nome|not))] | length' 2>/dev/null)
  if [ "${bad:-0}" -gt 0 ]; then Fl HONESTO "template com campo vazio" "$bad template(s) sem papéis/formatos"
  else P HONESTO "templates completos" "todos com papéis+formatos+metadados"; fi
  # 5e. perfis de teste residuais
  res=$(body GET /api/btv/users | jq '[.[]|select(.nome|test("^(SMOKE|FULL)·pin·"))]|length' 2>/dev/null)
  [ "${res:-0}" -gt 0 ] && W LEAK "perfis de teste residuais" "$res perfis “(SMOKE|FULL)·pin·…” (sem rota de delete)" || P LEAK "sem perfis residuais" "0"
else
  Inf HONESTO "ledger/verify" "$(body POST /api/ledger/verify)"
  Inf HONESTO "models/usage"  "$(body GET /api/models/usage | head -c 200)"
fi

echo "======================================================================"
printf 'RESUMO: %d PASS · %d WARN · %d INFO · %d FAIL\n' "$pass" "$warn" "$info" "$fail"
[ "$fail" -gt 0 ] && echo "❌ Há FAILs — corrigir." || true
[ "$warn" -gt 0 ] && echo "🟡 Há WARNs — decisões silenciosas para revisar." || true
exit 0
