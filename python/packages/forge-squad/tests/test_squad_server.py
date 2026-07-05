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

from forge_proto import core_pb2, core_pb2_grpc, llm_pb2, squad_pb2, squad_pb2_grpc

from forge_squad.server import SquadServicer


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
        self.permission_calls: list[str] = []

    async def Generate(self, request, context):  # noqa: N802
        self.generate_calls.append(request.requester)
        text = self.by_requester.get(request.requester, "{}")
        yield llm_pb2.LlmChunk(text_delta=text)
        yield llm_pb2.LlmChunk(usage=llm_pb2.Usage(input_tokens=1, output_tokens=2, cache_hit=False, provider="fake"))

    async def RequestPermission(self, request, context):  # noqa: N802
        self.permission_calls.append(request.tool)
        decision = core_pb2.PermissionDecision.ALLOW if self.permission_allow else core_pb2.PermissionDecision.DENY
        return core_pb2.PermissionDecision(decision=decision)


async def _run_scenario(tmp_path, arch_conf, dev_conf, aud_conf, approved, permission_allow):
    core_sock = str(tmp_path / "core.sock")
    squad_sock = str(tmp_path / "squad.sock")

    core = FakeCoreServicer(_responses(arch_conf, dev_conf, aud_conf, approved), permission_allow)
    core_server = grpc.aio.server()
    core_pb2_grpc.add_CoreServiceServicer_to_server(core, core_server)
    core_server.add_insecure_port(f"unix://{core_sock}")
    await core_server.start()

    squad_server = grpc.aio.server()
    squad_pb2_grpc.add_SquadServiceServicer_to_server(
        SquadServicer(core_socket=core_sock, memory_dir=tmp_path), squad_server
    )
    squad_server.add_insecure_port(f"unix://{squad_sock}")
    await squad_server.start()

    events = []
    async with grpc.aio.insecure_channel(f"unix://{squad_sock}") as ch:
        stub = squad_pb2_grpc.SquadServiceStub(ch)
        async for ev in stub.ExecuteTask(
            squad_pb2.SquadTask(task_id="t1", description="publicar serviço", decision_type="architecture")
        ):
            events.append(ev)

    await squad_server.stop(0)
    await core_server.stop(0)
    return events, core


def _kinds(events):
    return [ev.WhichOneof("payload") for ev in events]


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
