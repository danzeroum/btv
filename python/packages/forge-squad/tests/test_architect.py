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
    assert "Caching Layer" in result["plan"]["components"]
    assert "Accepted" in result["adr"]


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
    assert "Caching Layer" not in result["plan"]["components"]


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
