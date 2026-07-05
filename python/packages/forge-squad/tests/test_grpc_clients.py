"""Testes focados no mapeamento pydantic↔proto dos clients gRPC, contra um
`CoreService` real (in-process sobre UDS)."""

from __future__ import annotations

import asyncio

import grpc

from forge_proto import core_pb2, core_pb2_grpc, llm_pb2

from forge_squad.gateway import LlmRequest
from forge_squad.grpc_clients import GrpcGatewayClient, GrpcPermissionClient
from forge_squad.permission import PermissionRequest


class _CoreServicer(core_pb2_grpc.CoreServiceServicer):
    def __init__(self, allow: bool, note: str | None) -> None:
        self.allow = allow
        self.note = note
        self.last_request = None

    async def Generate(self, request, context):  # noqa: N802
        self.last_request = request
        yield llm_pb2.LlmChunk(text_delta="parte-1 ")
        yield llm_pb2.LlmChunk(text_delta="parte-2")
        yield llm_pb2.LlmChunk(
            usage=llm_pb2.Usage(input_tokens=10, output_tokens=5, cache_hit=True, provider="anthropic")
        )

    async def RequestPermission(self, request, context):  # noqa: N802
        self.last_request = request
        decision = core_pb2.PermissionDecision.ALLOW if self.allow else core_pb2.PermissionDecision.DENY
        msg = core_pb2.PermissionDecision(decision=decision)
        if self.note is not None:
            msg.operator_note = self.note
        return msg


async def _serve(tmp_path, servicer):
    sock = str(tmp_path / "core.sock")
    server = grpc.aio.server()
    core_pb2_grpc.add_CoreServiceServicer_to_server(servicer, server)
    server.add_insecure_port(f"unix://{sock}")
    await server.start()
    return server, sock


def test_gateway_agrega_chunks_e_usage(tmp_path):
    async def scenario():
        servicer = _CoreServicer(allow=True, note=None)
        server, sock = await _serve(tmp_path, servicer)
        async with grpc.aio.insecure_channel(f"unix://{sock}") as ch:
            client = GrpcGatewayClient(ch)
            resp = await client.generate(
                LlmRequest(model="claude-sonnet-5", messages=[{"role": "user", "content": "oi"}], requester="architect", temperature=0.3)
            )
        await server.stop(0)
        return resp, servicer

    resp, servicer = asyncio.run(scenario())
    assert resp.text == "parte-1 parte-2"  # deltas agregados na ordem
    assert resp.input_tokens == 10
    assert resp.output_tokens == 5
    assert resp.cache_hit is True
    assert resp.provider == "anthropic"
    # temperature opcional foi propagada; messages viraram messages_json.
    assert abs(servicer.last_request.temperature - 0.3) < 1e-9
    assert servicer.last_request.messages_json == '[{"role": "user", "content": "oi"}]'


def test_permission_allow_vira_approved_true_com_nota(tmp_path):
    async def scenario():
        servicer = _CoreServicer(allow=True, note="ok pelo operador")
        server, sock = await _serve(tmp_path, servicer)
        async with grpc.aio.insecure_channel(f"unix://{sock}") as ch:
            client = GrpcPermissionClient(ch)
            decision = await client.request_permission(
                PermissionRequest(tool="deploy", scope="ops", reason="crítico", confidence=0.4)
            )
        await server.stop(0)
        return decision

    decision = asyncio.run(scenario())
    assert decision.approved is True
    assert decision.operator_note == "ok pelo operador"


def test_permission_deny_vira_approved_false(tmp_path):
    async def scenario():
        servicer = _CoreServicer(allow=False, note=None)
        server, sock = await _serve(tmp_path, servicer)
        async with grpc.aio.insecure_channel(f"unix://{sock}") as ch:
            client = GrpcPermissionClient(ch)
            decision = await client.request_permission(
                PermissionRequest(tool="deploy", scope="ops", reason="crítico", confidence=0.4)
            )
        await server.stop(0)
        return decision

    decision = asyncio.run(scenario())
    # DENY (e também o default-zero DECISION_UNSPECIFIED) → approved False (fail-closed).
    assert decision.approved is False
    assert decision.operator_note is None
