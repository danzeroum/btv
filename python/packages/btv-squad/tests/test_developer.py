import asyncio
import json

from btv_squad import agents
from btv_squad.agents.developer import (
    _MAX_IDENTICAL_DENIED_REPEATS,
    _MAX_REACT_STEPS,
    _MAX_TOTAL_DENIALS,
    DeveloperAgent,
)
from btv_squad.gateway import LlmResponse, ScriptedGatewayClient
from btv_squad.tool_client import ScriptedToolClient, ToolCallResult


def test_execute_deriva_saida_real_do_gateway():
    payload = {
        "final_output": "def fatura(pedido): return pedido.total * 1.1",
        "status": "completed",
        "confidence": 0.82,
        "notes": "Falta tratar desconto promocional",
    }
    agent = DeveloperAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))

    result = asyncio.run(agent.execute({"description": "calcular fatura com imposto"}))

    assert result["success"] is True
    # Igualdade, não só presença: prova pass-through fiel.
    assert result["final_output"] == "def fatura(pedido): return pedido.total * 1.1"
    assert result["status"] == "completed"
    assert result["confidence"] == 0.82
    assert result["notes"] == "Falta tratar desconto promocional"


def test_dois_pedidos_diferentes_produzem_saidas_diferentes():
    payload_a = {"final_output": "codigo A", "status": "completed", "confidence": 0.9}
    payload_b = {"final_output": "codigo B", "status": "incomplete", "confidence": 0.4}
    agent = DeveloperAgent()
    agent.attach_gateway(
        ScriptedGatewayClient([LlmResponse(text=json.dumps(payload_a)), LlmResponse(text=json.dumps(payload_b))])
    )

    result_a = asyncio.run(agent.execute({"description": "tarefa A"}))
    result_b = asyncio.run(agent.execute({"description": "tarefa B"}))

    assert result_a["final_output"] == "codigo A"
    assert result_b["final_output"] == "codigo B"


def test_execute_sem_gateway_levanta_erro_claro():
    agent = DeveloperAgent()
    try:
        asyncio.run(agent.execute({"description": "tarefa qualquer"}))
        assert False, "deveria ter levantado RuntimeError"
    except RuntimeError as exc:
        assert "attach_gateway" in str(exc)


def test_resposta_sem_json_cai_no_fallback_honesto():
    agent = DeveloperAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text="não consigo processar essa tarefa.")]))

    result = asyncio.run(agent.execute({"description": "tarefa"}))

    assert result["final_output"] == ""
    assert result["status"] == "incomplete"
    assert result["confidence"] == 0.0


def test_generate_code_sem_review_system_devolve_codigo_sem_revisao():
    payload = {"final_output": "print('oi')", "status": "completed", "confidence": 0.7}
    agent = DeveloperAgent()  # review_system=None por padrão
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))

    code = asyncio.run(agent.generate_code({"description": "hello world"}))

    assert code == "print('oi')"


def test_generate_code_com_review_system_aprovado_usa_codigo_revisado():
    payload = {"final_output": "print('oi')", "status": "completed", "confidence": 0.7}

    class _ApprovingReview:
        async def review_code(self, code, metadata):
            return {"approved": True, "code": "print('oi revisado')"}

    agent = DeveloperAgent(review_system=_ApprovingReview())
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))

    code = asyncio.run(agent.generate_code({"description": "hello world"}))

    assert code == "print('oi revisado')"


def test_execute_com_action_usa_tool_client_e_faz_tool_call_antes_do_final_answer():
    tool_call_turn = json.dumps(
        {"action": "tool_call", "tool": "bash", "args": {"command": "echo oi > out.txt"}}
    )
    final_turn = json.dumps(
        {
            "action": "final_answer",
            "final_output": "arquivo criado",
            "status": "completed",
            "confidence": 0.9,
            "notes": "verificado com cat",
        }
    )
    agent = DeveloperAgent()
    agent.attach_gateway(
        ScriptedGatewayClient([LlmResponse(text=tool_call_turn), LlmResponse(text=final_turn)])
    )
    tool_client = ScriptedToolClient([ToolCallResult(content="oi\n", exit_code=0)])
    agent.attach_tool_client(tool_client)

    result = asyncio.run(agent.execute({"description": "crie out.txt", "action": "implement"}))

    assert len(tool_client.requests) == 1
    assert tool_client.requests[0].tool == "bash"
    assert json.loads(tool_client.requests[0].args_json) == {"command": "echo oi > out.txt"}
    assert result["status"] == "completed"
    assert result["final_output"] == "arquivo criado"
    assert len(result["tool_calls"]) == 1
    assert result["tool_calls"][0]["exit_code"] == 0


def test_execute_sem_action_nao_usa_tools_mesmo_com_tool_client_anexado():
    payload = {"final_output": "código", "status": "completed", "confidence": 0.8}
    agent = DeveloperAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))
    tool_client = ScriptedToolClient([ToolCallResult(content="não deveria ser chamado")])
    agent.attach_tool_client(tool_client)

    # Estilo proposta/avaliação (`_get_squad_proposals`) — sem "action".
    result = asyncio.run(agent.execute({"description": "avalie a tarefa X"}))

    assert tool_client.requests == []
    assert result["final_output"] == "código"
    assert "tool_calls" not in result  # caminho antigo de chamada única, formato intacto


def test_react_loop_esgota_passos_sem_final_answer_devolve_incomplete_honesto():
    tool_call_turn = json.dumps({"action": "tool_call", "tool": "bash", "args": {"command": "ls"}})
    agent = DeveloperAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=tool_call_turn)] * _MAX_REACT_STEPS))
    tool_client = ScriptedToolClient(
        [ToolCallResult(content="arquivo.txt", exit_code=0) for _ in range(_MAX_REACT_STEPS)]
    )
    agent.attach_tool_client(tool_client)

    result = asyncio.run(agent.execute({"description": "tarefa sem fim", "action": "implement"}))

    assert result["status"] == "incomplete"
    assert result["final_output"] == ""
    assert len(result["tool_calls"]) == _MAX_REACT_STEPS


def test_denial_budget_interrompe_loop_de_negacoes_identicas():
    """PATCH ciclo completo: o sintoma real (modelo repetindo o MESMO caminho
    negado até o teto de 600s) agora interrompe em poucas tentativas, com
    causa específica no `notes` e o rastro de tool_calls preservado."""
    tool_call_turn = json.dumps(
        {"action": "tool_call", "tool": "bash", "args": {"command": "mkdir -p /tmp/composicao_rock"}}
    )
    agent = DeveloperAgent()
    agent.attach_gateway(
        ScriptedGatewayClient([LlmResponse(text=tool_call_turn)] * (_MAX_IDENTICAL_DENIED_REPEATS + 1))
    )
    negada = ToolCallResult(
        content="permissão negada",
        exit_code=-1,
        recovery_hint="use caminhos relativos a '/work'",
    )
    agent.attach_tool_client(ScriptedToolClient([negada] * (_MAX_IDENTICAL_DENIED_REPEATS + 1)))

    result = asyncio.run(agent.execute({"description": "crie a composição", "action": "implement"}))

    assert result["status"] == "incomplete"
    assert "negada pelo motor de permissões" in result["notes"]
    assert "/work" in result["notes"], "recovery_hint entra no notes"
    assert len(result["tool_calls"]) == _MAX_IDENTICAL_DENIED_REPEATS + 1
    assert result["tool_calls"][0]["recovery_hint"] == "use caminhos relativos a '/work'"


def test_denial_budget_total_interrompe_negacoes_diferentes():
    # Caminhos DIFERENTES a cada negação: só o orçamento total (4) dispara —
    # o orçamento de idênticas (2 consecutivas) nunca é alcançado.
    agent = DeveloperAgent()
    agent.attach_gateway(
        ScriptedGatewayClient(
            [
                LlmResponse(
                    text=json.dumps(
                        {"action": "tool_call", "tool": "bash", "args": {"command": f"pwd {i}"}}
                    )
                )
                for i in range(_MAX_TOTAL_DENIALS + 1)
            ]
        )
    )
    agent.attach_tool_client(
        ScriptedToolClient(
            [
                ToolCallResult(content=f"negada {i}", exit_code=-1, recovery_hint="hint")
                for i in range(_MAX_TOTAL_DENIALS + 1)
            ]
        )
    )

    result = asyncio.run(agent.execute({"description": "tarefa", "action": "implement"}))

    assert result["status"] == "incomplete"
    assert len(result["tool_calls"]) == _MAX_TOTAL_DENIALS + 1


def test_timeout_preserva_tool_calls_parciais(monkeypatch):
    """PATCH ciclo completo: o timeout de 600s NÃO apaga mais o rastro do que
    já rodou de verdade — a evidência parcial sobrevive para o auditor."""
    tool_call_turn = json.dumps({"action": "tool_call", "tool": "bash", "args": {"command": "ls"}})

    class _HangingGateway:
        def __init__(self) -> None:
            self.calls = 0

        async def generate(self, request) -> LlmResponse:
            self.calls += 1
            if self.calls == 1:
                return LlmResponse(text=tool_call_turn)
            await asyncio.sleep(3600)
            raise AssertionError("não deveria voltar")

    monkeypatch.setattr(agents.developer, "_REACT_TIMEOUT_SECONDS", 0.2)
    agent = DeveloperAgent()
    agent.attach_gateway(_HangingGateway())
    agent.attach_tool_client(ScriptedToolClient([ToolCallResult(content="arquivo.txt", exit_code=0)]))

    result = asyncio.run(agent.execute({"description": "tarefa", "action": "implement"}))

    assert result["status"] == "incomplete"
    assert "600s" in result["notes"] or "excedeu" in result["notes"]
    assert len(result["tool_calls"]) == 1, "tool_call executada ANTES do timeout é preservada"
    assert result["tool_calls"][0]["exit_code"] == 0
