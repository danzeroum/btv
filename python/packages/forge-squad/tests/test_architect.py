import asyncio
import json

from forge_squad.agents.architect import ArchitectAgent
from forge_squad.gateway import LlmResponse, ScriptedGatewayClient


def _well_formed_response() -> LlmResponse:
    payload = {
        "problem_analysis": "API sem cache está sofrendo com P95 alto",
        "constraints": ["P95 < 800ms", "Compatibilidade retroativa"],
        "applicable_patterns": ["Cache-aside", "API Gateway"],
        "trade_offs": {"Cache-aside": "Simplicidade, mas dados podem ficar obsoletos"},
        "recommendation": "Introduzir uma camada de cache na frente da API",
        "architecture": "monolito modular",
        "components": ["Cache Redis", "API HTTP", "Serviço de faturamento"],
        "risks": ["Dados obsoletos sob invalidação incorreta"],
        "mitigations": ["TTL curto", "Invalidação ativa no write"],
        "estimated_effort": "1 sprint",
        "confidence": 0.88,
    }
    return LlmResponse(text=json.dumps(payload))


def test_execute_chama_o_gateway_e_produz_decisao_real():
    agent = ArchitectAgent()
    agent.attach_gateway(ScriptedGatewayClient([_well_formed_response()]))

    result = asyncio.run(agent.execute({"description": "API lenta sob carga"}))

    assert result["success"] is True
    assert result["confidence"] == 0.88
    assert "cache" in result["reasoning"]["recommendation"].lower()
    # O plano inteiro vem do modelo, não de uma lista fixa — "Serviço de
    # faturamento" não existe em nenhuma constante do código, só na
    # resposta roteirizada. Se aparecer aqui, é prova de derivação real.
    assert result["plan"]["architecture"] == "monolito modular"
    assert "Serviço de faturamento" in result["plan"]["components"]
    assert result["plan"]["risks"] == ["Dados obsoletos sob invalidação incorreta"]
    assert result["plan"]["estimated_effort"] == "1 sprint"
    assert "Accepted" in result["adr"]


def test_dois_problemas_diferentes_produzem_planos_diferentes():
    # Trava o bug que este teste substitui: antes, create_plan devolvia a
    # mesma lista fixa de componentes para qualquer problema.
    payload_a = {
        "recommendation": "cache",
        "architecture": "monolito",
        "components": ["Redis"],
        "confidence": 0.7,
    }
    payload_b = {
        "recommendation": "filas",
        "architecture": "event-driven",
        "components": ["Kafka", "Consumer Group"],
        "confidence": 0.7,
    }
    agent = ArchitectAgent()
    agent.attach_gateway(
        ScriptedGatewayClient([LlmResponse(text=json.dumps(payload_a)), LlmResponse(text=json.dumps(payload_b))])
    )

    plan_a = asyncio.run(agent.execute({"description": "problema A"}))["plan"]
    plan_b = asyncio.run(agent.execute({"description": "problema B"}))["plan"]

    # Igualdade, não só diferença: prova pass-through fiel da saída do
    # modelo, não apenas que o plano "varia" (o que passaria mesmo se
    # create_plan transformasse os valores em vez de repassá-los).
    assert plan_a["architecture"] == "monolito"
    assert plan_a["components"] == ["Redis"]
    assert plan_b["architecture"] == "event-driven"
    assert plan_b["components"] == ["Kafka", "Consumer Group"]


def test_execute_sem_gateway_anexado_levanta_erro_claro():
    agent = ArchitectAgent()
    try:
        asyncio.run(agent.execute({"description": "tarefa qualquer"}))
        assert False, "deveria ter levantado RuntimeError"
    except RuntimeError as exc:
        assert "attach_gateway" in str(exc)


def test_resposta_do_modelo_sem_json_cai_no_fallback_defensivo():
    agent = ArchitectAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text="desculpe, não consigo ajudar com isso.")]))

    result = asyncio.run(agent.execute({"description": "problema qualquer"}))

    assert result["success"] is True
    assert result["confidence"] == 0.0
    assert result["reasoning"]["recommendation"] == ""
    # Fallback honesto: plano vazio (baixa confiança), não um plano
    # genérico fabricado por engano.
    assert result["plan"]["components"] == []
    assert result["plan"]["architecture"] == ""


def test_json_com_texto_ao_redor_ainda_e_parseado():
    payload = {
        "problem_analysis": "x",
        "recommendation": "usar fila assíncrona",
        "confidence": 0.6,
    }
    wrapped = f"Aqui está minha análise:\n```json\n{json.dumps(payload)}\n```\nEspero que ajude."
    agent = ArchitectAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=wrapped)]))

    result = asyncio.run(agent.execute({"description": "problema"}))

    assert result["confidence"] == 0.6
    assert result["reasoning"]["recommendation"] == "usar fila assíncrona"


def test_reasoning_history_acumula_entre_chamadas():
    agent = ArchitectAgent()
    agent.attach_gateway(ScriptedGatewayClient([_well_formed_response(), _well_formed_response()]))

    asyncio.run(agent.execute({"description": "primeiro problema"}))
    asyncio.run(agent.execute({"description": "segundo problema"}))

    assert len(agent.reasoning_history) == 2
