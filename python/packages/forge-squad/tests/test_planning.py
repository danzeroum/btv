import asyncio
import json

from forge_squad.gateway import LlmResponse, ScriptedGatewayClient
from forge_squad.planning import AdaptivePlanner


def _decomposition_payload(**overrides):
    payload = {
        "steps": [
            {"step": 1, "action": "analyze", "description": "Mapear integrações do gateway de pagamento", "estimated_time": 5, "dependencies": [], "can_fail": True},
            {"step": 2, "action": "implement", "description": "Implementar webhook de confirmação", "estimated_time": 20, "dependencies": [1], "can_fail": True},
            {"step": 3, "action": "validate", "description": "Testar cenários de falha do provedor", "estimated_time": 8, "dependencies": [2], "can_fail": False},
        ],
        "estimated_duration": 33,
        "confidence": 0.77,
    }
    payload.update(overrides)
    return payload


def test_create_adaptive_plan_deriva_passos_reais_do_gateway():
    planner = AdaptivePlanner()
    planner.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(_decomposition_payload()))]))

    plan = asyncio.run(planner.create_adaptive_plan({"description": "integrar gateway de pagamento"}))

    assert plan["steps"][1]["description"] == "Implementar webhook de confirmação"
    assert plan["estimated_duration"] == 33
    assert plan["confidence"] == 0.77
    assert plan["adaptive"] is True


def test_duas_tarefas_diferentes_produzem_planos_diferentes():
    payload_a = _decomposition_payload(estimated_duration=10, confidence=0.9)
    payload_b = _decomposition_payload(
        steps=[{"step": 1, "action": "design", "description": "Redesenhar dashboard", "estimated_time": 40, "dependencies": [], "can_fail": True}],
        estimated_duration=40,
        confidence=0.5,
    )
    planner = AdaptivePlanner()
    planner.attach_gateway(
        ScriptedGatewayClient([LlmResponse(text=json.dumps(payload_a)), LlmResponse(text=json.dumps(payload_b))])
    )

    plan_a = asyncio.run(planner.create_adaptive_plan({"description": "tarefa A"}))
    plan_b = asyncio.run(planner.create_adaptive_plan({"description": "tarefa B"}))

    assert plan_a["estimated_duration"] == 10
    assert plan_b["estimated_duration"] == 40
    assert plan_b["steps"][0]["description"] == "Redesenhar dashboard"


def test_create_adaptive_plan_sem_gateway_levanta_erro_claro():
    planner = AdaptivePlanner()
    try:
        asyncio.run(planner.create_adaptive_plan({"description": "tarefa"}))
        assert False, "deveria ter levantado RuntimeError"
    except RuntimeError as exc:
        assert "attach_gateway" in str(exc)


def test_resposta_sem_json_cai_no_fallback_honesto():
    planner = AdaptivePlanner()
    planner.attach_gateway(ScriptedGatewayClient([LlmResponse(text="não consigo planejar isso.")]))

    plan = asyncio.run(planner.create_adaptive_plan({"description": "tarefa"}))

    assert plan["steps"] == []
    assert plan["estimated_duration"] == 0
    assert plan["confidence"] == 0.0


def test_replan_from_point_deriva_passos_de_recuperacao_reais():
    planner = AdaptivePlanner()
    planner.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(_decomposition_payload()))]))
    plan = asyncio.run(planner.create_adaptive_plan({"description": "tarefa"}))

    recovery_payload = {
        "recovery_steps": [
            {"action": "implement", "description": "Adicionar retry com backoff exponencial", "estimated_time": 15, "can_fail": True},
        ],
        "confidence_penalty": 0.2,
    }
    planner.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(recovery_payload))]))
    failed_step = plan["steps"][1]  # step 2, "implement"
    reflection = {"reason": "timeout", "suggestion": "increase_timeout"}

    new_plan = asyncio.run(planner.replan_from_point(plan, failed_step, reflection))

    descriptions = [s["description"] for s in new_plan["steps"]]
    assert "Adicionar retry com backoff exponencial" in descriptions
    assert new_plan["confidence"] == plan["confidence"] - 0.2
    assert new_plan["replanned"] is True
    # Passos concluídos antes do que falhou continuam intactos; o restante é renumerado.
    assert new_plan["steps"][0]["description"] == plan["steps"][0]["description"]


def test_analyze_failure_classifica_timeout_deterministicamente():
    planner = AdaptivePlanner()
    result = planner.analyze_failure(TimeoutError("connection timeout after 30s"), {"plan_id": "p1"})
    assert result["reason"] == "timeout"
    assert result["suggestion"] == "increase_timeout"


def test_analyze_failure_classifica_erro_desconhecido():
    planner = AdaptivePlanner()
    result = planner.analyze_failure(ValueError("invalid input"), {"plan_id": "p1"})
    assert result["reason"] == "unknown"
