"""Teste do laço gRPC bidirecional do squad, todo em Python:

  cliente SquadService  →  SquadService (real)  →  CoreService (fake, papel do Rust)

Prova a promessa do ADR 0005 (os agentes recebem LLM/permissão via
`CoreService`, sem mudar) e que o mapeamento pydantic↔proto não perde
campo — em especial `Consensus.requires_human`, que o default-zero do
proto3 apagaria silenciosamente se não fosse setado à mão.
"""

from __future__ import annotations

import asyncio
import json

import grpc

from btv_proto import core_pb2, core_pb2_grpc, llm_pb2, squad_pb2, squad_pb2_grpc

from btv_squad.server import SquadServicer


def _responses(arch_conf: float, dev_conf: float, aud_conf: float, approved: bool) -> dict[str, str]:
    return {
        "planner": json.dumps(
            {
                "steps": [
                    {"step": 1, "action": "deploy", "description": "publicar", "estimated_time": 10, "dependencies": [], "can_fail": True}
                ],
                "estimated_duration": 10,
                "confidence": 0.8,
            }
        ),
        "architect": json.dumps(
            {"problem_analysis": "x", "recommendation": "micro", "architecture": "microservices", "components": ["api"], "confidence": arch_conf}
        ),
        "developer": json.dumps({"final_output": "code", "status": "completed", "confidence": dev_conf}),
        "auditor": json.dumps(
            {"passed": approved, "approved": approved, "confidence": aud_conf, "notes": "ok", "issues": [], "agent_scores": {}, "additional_checks": []}
        ),
        "designer": json.dumps({"pattern": "material", "components": ["ui"], "confidence": 0.8}),
        "ops": json.dumps({"strategy": "blue-green", "stages": ["build"], "confidence": 0.9}),
    }


class FakeCoreServicer(core_pb2_grpc.CoreServiceServicer):
    """Papel do Rust: Generate roteia por requester; RequestPermission decide."""

    def __init__(self, by_requester: dict[str, str], permission_allow: bool) -> None:
        self.by_requester = by_requester
        self.permission_allow = permission_allow
        self.generate_calls: list[str] = []
        self.generate_models: list[str] = []
        self.permission_calls: list[str] = []

    async def Generate(self, request, context):  # noqa: N802
        self.generate_calls.append(request.requester)
        self.generate_models.append(request.model)
        text = self.by_requester.get(request.requester, "{}")
        yield llm_pb2.LlmChunk(text_delta=text)
        yield llm_pb2.LlmChunk(usage=llm_pb2.Usage(input_tokens=1, output_tokens=2, cache_hit=False, provider="fake"))

    async def RequestPermission(self, request, context):  # noqa: N802
        self.permission_calls.append(request.tool)
        decision = core_pb2.PermissionDecision.ALLOW if self.permission_allow else core_pb2.PermissionDecision.DENY
        return core_pb2.PermissionDecision(decision=decision)


async def _run_scenario(
    tmp_path,
    arch_conf,
    dev_conf,
    aud_conf,
    approved,
    permission_allow,
    verification_evidence=None,
    model="",
    pool_model="claude-sonnet-5",
):
    core_sock = str(tmp_path / "core.sock")
    squad_sock = str(tmp_path / "squad.sock")

    core = FakeCoreServicer(_responses(arch_conf, dev_conf, aud_conf, approved), permission_allow)
    core_server = grpc.aio.server()
    core_pb2_grpc.add_CoreServiceServicer_to_server(core, core_server)
    core_server.add_insecure_port(f"unix://{core_sock}")
    await core_server.start()

    squad_server = grpc.aio.server()
    squad_pb2_grpc.add_SquadServiceServicer_to_server(
        SquadServicer(core_socket=core_sock, model=pool_model, memory_dir=tmp_path), squad_server
    )
    squad_server.add_insecure_port(f"unix://{squad_sock}")
    await squad_server.start()

    events = []
    async with grpc.aio.insecure_channel(f"unix://{squad_sock}") as ch:
        stub = squad_pb2_grpc.SquadServiceStub(ch)
        async for ev in stub.ExecuteTask(
            squad_pb2.SquadTask(
                task_id="t1",
                description="publicar serviço",
                decision_type="architecture",
                verification_evidence=verification_evidence,
                model=model,
            )
        ):
            events.append(ev)

    await squad_server.stop(0)
    await core_server.stop(0)
    return events, core


def _kinds(events):
    return [ev.WhichOneof("payload") for ev in events]


def test_model_por_tarefa_sobrepoe_o_default_do_pool(tmp_path):
    """O `model` da SquadTask (tela Modelo / --model) chega a CADA agente:
    todas as chamadas Generate ao CoreService usam o modelo da tarefa, não o
    default do pool."""
    _events, core = asyncio.run(
        _run_scenario(
            tmp_path, 0.9, 0.2, 0.2, approved=True, permission_allow=True,
            model="deepseek-chat", pool_model="claude-sonnet-5",
        )
    )
    assert core.generate_models, "esperava chamadas Generate"
    assert all(m == "deepseek-chat" for m in core.generate_models), (
        f"todo Generate deveria usar o modelo da tarefa; veio: {set(core.generate_models)}"
    )


def test_model_vazio_na_tarefa_herda_o_default_do_pool(tmp_path):
    _events, core = asyncio.run(
        _run_scenario(
            tmp_path, 0.9, 0.2, 0.2, approved=True, permission_allow=True,
            model="", pool_model="claude-sonnet-5",
        )
    )
    assert all(m == "claude-sonnet-5" for m in core.generate_models), (
        f"model vazio deveria herdar o default do pool; veio: {set(core.generate_models)}"
    )


def test_stream_de_eventos_com_consenso_forte(tmp_path):
    events, core = asyncio.run(_run_scenario(tmp_path, 0.9, 0.2, 0.2, approved=True, permission_allow=True))
    kinds = _kinds(events)
    assert "proposal" in kinds
    assert "consensus" in kinds
    assert "step" in kinds

    consensus_ev = next(ev for ev in events if ev.WhichOneof("payload") == "consensus")
    # consenso forte → requires_human False, preservado no proto.
    assert consensus_ev.consensus.requires_human is False
    assert consensus_ev.consensus.decision_maker == "architect"

    # 3 propostas (architect/developer/auditor), cada uma com content_json real.
    proposals = [ev.proposal for ev in events if ev.WhichOneof("payload") == "proposal"]
    assert {p.agent for p in proposals} == {"architect", "developer", "auditor"}


def test_chat_message_atravessa_o_mapeamento_proto(tmp_path):
    # Fase 1: os agentes ganham voz. Os eventos "chat" viram ChatMessage no
    # proto, com author/author_role/text preenchidos (não default-zero).
    events, _core = asyncio.run(_run_scenario(tmp_path, 0.9, 0.2, 0.2, approved=True, permission_allow=True))
    chats = [ev.chat for ev in events if ev.WhichOneof("payload") == "chat"]
    assert chats, "esperava ao menos um ChatMessage no stream"
    assert all(c.text for c in chats)
    assert all(c.author_role in ("AGENT", "HUMAN", "SYSTEM") for c in chats)
    # há narração dos agentes (AGENT) e do próprio squad no consenso (SYSTEM).
    assert {c.author_role for c in chats} >= {"AGENT", "SYSTEM"}


def test_requires_human_true_sobrevive_ao_mapeamento_proto(tmp_path):
    # Consenso fraco → requires_human True. Se o mapeamento pydantic→proto
    # não setasse o campo à mão, o default-zero do proto3 devolveria False.
    events, core = asyncio.run(_run_scenario(tmp_path, 0.6, 0.6, 0.9, approved=True, permission_allow=True))
    consensus_ev = next(ev for ev in events if ev.WhichOneof("payload") == "consensus")
    assert consensus_ev.consensus.requires_human is True
    hitl = [ev for ev in events if ev.WhichOneof("payload") == "hitl"]
    assert len(hitl) == 1
    assert core.permission_calls  # o HITL de fato chamou CoreService.RequestPermission


def test_agentes_recebem_llm_via_coreservice(tmp_path):
    # ADR 0005: os agentes/planner obtêm o LLM de volta do Core — prova que
    # GrpcGatewayClient fechou o laço (os Scripted não existem aqui).
    _events, core = asyncio.run(_run_scenario(tmp_path, 0.9, 0.2, 0.2, approved=True, permission_allow=True))
    assert "planner" in core.generate_calls
    assert "architect" in core.generate_calls
    assert "auditor" in core.generate_calls


def test_hitl_negado_aborta_o_stream_sem_steps(tmp_path):
    events, core = asyncio.run(_run_scenario(tmp_path, 0.6, 0.6, 0.9, approved=True, permission_allow=False))
    kinds = _kinds(events)
    assert "consensus" in kinds
    assert "hitl" in kinds
    assert "step" not in kinds  # negado antes de executar qualquer passo
    assert core.permission_calls


def test_handoff_start_e_complete_aparecem(tmp_path):
    events, _core = asyncio.run(_run_scenario(tmp_path, 0.9, 0.2, 0.2, approved=True, permission_allow=True))
    handoffs = [ev.handoff for ev in events if ev.WhichOneof("payload") == "handoff"]
    phases = {h.phase for h in handoffs}
    assert squad_pb2.Handoff.Phase.START in phases
    assert squad_pb2.Handoff.Phase.COMPLETE in phases


# --- Fase 5 Onda 3 (D3t): verification_evidence atravessa a fronteira gRPC ---
# (ADR 0008/0030). Antes uma string JSON (`verification_evidence_json`); agora
# mensagem TIPADA. Campo de mensagem em proto3 tem PRESENÇA (`HasField`) — sem
# tratamento explícito no server.py, ausente viraria silenciosamente "sem
# evidência, tudo bem". Os três testes abaixo provam por CONTAGEM DE CHAMADAS
# ao gateway (via o FakeCoreServicer real, não um mock que finge) que o campo
# de fato atravessou Rust→Python e mudou o comportamento do orquestrador:
# ausente OU inválida (verdict UNSPECIFIED) ⇒ fail-closed sem gastar uma chamada
# de LLM em validate_results; presente e válida ⇒ o fluxo normal roda.


def test_verification_evidence_ausente_e_fail_closed_sem_chamar_validate_results(tmp_path):
    # SquadTask sem verification_evidence (campo de mensagem não setado,
    # HasField falso) deve fazer o orquestrador reprovar SEM sequer chamar o
    # gateway para validate_results — só a chamada de execute() (proposta).
    events, core = asyncio.run(_run_scenario(tmp_path, 0.9, 0.2, 0.2, approved=True, permission_allow=True))
    assert core.generate_calls.count("auditor") == 1  # só a proposta, não validate_results
    assert "step" in _kinds(events)  # os passos ainda executam — só a validação final é que reprova


def test_verification_evidence_valida_permite_validate_results_normal(tmp_path):
    evidence = squad_pb2.VerificationEvidence(
        run_id="r1",
        git_sha="deadbeef",
        verdict=squad_pb2.Verdict.VERDICT_PASS,
        produced_at="2026-01-01T00:00:00Z",
    )
    events, core = asyncio.run(
        _run_scenario(tmp_path, 0.9, 0.2, 0.2, approved=True, permission_allow=True, verification_evidence=evidence)
    )
    # com evidência válida, validate_results roda de verdade: 2 chamadas ao
    # auditor (proposta em execute() + validação final em validate_results()).
    assert core.generate_calls.count("auditor") == 2
    assert "step" in _kinds(events)


def test_verification_evidence_verdict_invalido_tambem_e_fail_closed(tmp_path):
    # Mensagem PRESENTE mas com verdict UNSPECIFIED (zero-value proto3) — a
    # analogia tipada do "JSON inválido" de antes: evidência sem veredito válido
    # é recusada fail-closed (`from_proto` levanta ValueError), mesmo padrão do
    # campo ausente.
    evidence = squad_pb2.VerificationEvidence(run_id="r1", git_sha="deadbeef", produced_at="2026-01-01T00:00:00Z")
    events, core = asyncio.run(
        _run_scenario(tmp_path, 0.9, 0.2, 0.2, approved=True, permission_allow=True, verification_evidence=evidence)
    )
    assert core.generate_calls.count("auditor") == 1  # fail-closed, mesmo padrão do campo ausente
