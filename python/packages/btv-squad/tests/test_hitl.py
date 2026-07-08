import asyncio

import pytest

from btv_squad.hitl import ProgressiveAutonomyManager
from btv_squad.permission import PermissionDecision, ScriptedPermissionClient


def test_agente_novo_comeca_no_nivel_1_e_exige_aprovacao():
    manager = ProgressiveAutonomyManager()
    manager.attach_permission_client(ScriptedPermissionClient([PermissionDecision(approved=True)]))

    result = asyncio.run(manager.execute_with_autonomy("architect", {"action": "approve_plan"}))

    assert result["executed"] is True


def test_aprovacao_negada_marca_nao_executado_e_penaliza_confianca():
    manager = ProgressiveAutonomyManager()
    manager.attach_permission_client(
        ScriptedPermissionClient([PermissionDecision(approved=False, operator_note="risco demais")])
    )

    result = asyncio.run(manager.execute_with_autonomy("architect", {"action": "approve_plan"}))

    assert result["executed"] is False
    assert result["feedback"] == "risco demais"
    assert manager.agent_trust_scores["architect"] == pytest.approx(0.4)  # 0.5 - 0.1


def test_agente_com_autonomia_total_nao_precisa_de_permission_client():
    manager = ProgressiveAutonomyManager(agent_trust_scores={"ops": 0.9})  # nível 3

    result = asyncio.run(manager.execute_with_autonomy("ops", {"action": "deploy", "critical": True}))

    assert result["executed"] is True  # nunca chamou o permission_client


def test_nivel_2_so_exige_aprovacao_para_acao_critica():
    manager = ProgressiveAutonomyManager(agent_trust_scores={"developer": 0.7})  # nível 2

    non_critical = asyncio.run(manager.execute_with_autonomy("developer", {"action": "write_code"}))
    assert non_critical["executed"] is True  # sem permission_client e sem erro

    manager.attach_permission_client(ScriptedPermissionClient([PermissionDecision(approved=True)]))
    critical = asyncio.run(manager.execute_with_autonomy("developer", {"action": "deploy_prod", "critical": True}))
    assert critical["executed"] is True


def test_aprovacao_necessaria_sem_permission_client_levanta_erro_claro():
    manager = ProgressiveAutonomyManager()
    try:
        asyncio.run(manager.execute_with_autonomy("architect", {"action": "approve_plan"}))
        assert False, "deveria ter levantado RuntimeError"
    except RuntimeError as exc:
        assert "attach_permission_client" in str(exc)


def test_record_action_publico_atualiza_score_com_resultado_real():
    manager = ProgressiveAutonomyManager()
    manager.record_action("developer", {"action": "write_code"}, success=True)
    assert manager.agent_trust_scores["developer"] == pytest.approx(0.52)  # 0.5 + 0.02

    manager.record_action("developer", {"action": "write_code"}, success=False)
    assert manager.agent_trust_scores["developer"] == pytest.approx(0.42)  # 0.52 - 0.1


def test_score_e_limitado_entre_0_e_1():
    manager = ProgressiveAutonomyManager(agent_trust_scores={"x": 0.99})
    manager.record_action("x", {}, success=True)
    assert manager.agent_trust_scores["x"] == 1.0

    manager.agent_trust_scores["y"] = 0.05
    manager.record_action("y", {}, success=False)
    assert manager.agent_trust_scores["y"] == 0.0
