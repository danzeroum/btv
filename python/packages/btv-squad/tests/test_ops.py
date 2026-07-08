import asyncio
import json

from btv_squad.agents.ops import OpsAgent
from btv_squad.gateway import LlmResponse, ScriptedGatewayClient


def _plan_payload(**overrides):
    payload = {
        "strategy": "canary",
        "stages": ["build", "canary-5pct", "canary-50pct", "production"],
        "rollback_plan": True,
        "health_checks": ["grpc-health", "latencia-p99"],
        "scaling": {"min_instances": 3, "max_instances": 20, "target_cpu": 60},
        "monitoring": {
            "metrics": ["latencia", "taxa_erro"],
            "alerts": [{"metric": "taxa_erro", "threshold": 0.02, "action": "rollback"}],
            "dashboards": ["overview"],
            "logging": {"level": "warn", "structured": True, "retention_days": 14},
        },
        "confidence": 0.85,
        "notes": "Serviço crítico — canary conservador com rollback automático",
    }
    payload.update(overrides)
    return payload


def test_execute_deriva_plano_real_do_gateway():
    agent = OpsAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(_plan_payload()))]))

    result = asyncio.run(agent.execute({"description": "deploy do serviço de pagamentos"}))

    assert result["success"] is True
    assert result["strategy"] == "canary"
    assert result["stages"] == ["build", "canary-5pct", "canary-50pct", "production"]
    assert result["scaling"]["max_instances"] == 20
    assert result["monitoring"]["alerts"][0]["action"] == "rollback"
    assert result["confidence"] == 0.85


def test_estrategia_nao_suportada_cai_para_blue_green():
    agent = OpsAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(_plan_payload(strategy="big-bang")))]))

    result = asyncio.run(agent.execute({"description": "deploy simples"}))

    assert result["strategy"] == "blue-green"


def test_dois_servicos_diferentes_produzem_planos_diferentes():
    agent = OpsAgent()
    agent.attach_gateway(
        ScriptedGatewayClient(
            [
                LlmResponse(text=json.dumps(_plan_payload(strategy="rolling", scaling={"min_instances": 1}))),
                LlmResponse(text=json.dumps(_plan_payload(strategy="canary", scaling={"min_instances": 10}))),
            ]
        )
    )

    result_a = asyncio.run(agent.execute({"description": "serviço interno de baixo tráfego"}))
    result_b = asyncio.run(agent.execute({"description": "serviço público de alto tráfego"}))

    assert result_a["strategy"] == "rolling"
    assert result_a["scaling"] == {"min_instances": 1}
    assert result_b["strategy"] == "canary"
    assert result_b["scaling"] == {"min_instances": 10}


def test_execute_sem_gateway_levanta_erro_claro():
    agent = OpsAgent()
    try:
        asyncio.run(agent.execute({"description": "tarefa qualquer"}))
        assert False, "deveria ter levantado RuntimeError"
    except RuntimeError as exc:
        assert "attach_gateway" in str(exc)


def test_resposta_sem_json_cai_no_fallback_honesto():
    agent = OpsAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text="não consigo planejar esse deploy.")]))

    result = asyncio.run(agent.execute({"description": "tarefa"}))

    assert result["stages"] == []
    assert result["health_checks"] == []
    assert result["confidence"] == 0.0
    assert result["strategy"] == "blue-green"  # default determinístico da guarda
