"""Testa o servidor gRPC real (grpc.aio) sobre um Unix Domain Socket
efêmero — sem pytest-asyncio, via asyncio.run em cada teste."""

from __future__ import annotations

import asyncio

import grpc
import pytest

from btv_proto import promptforge_pb2, promptforge_pb2_grpc
from btv_promptforge.server import VERSION, PromptForgeServicer


async def _with_server(socket_path: str, body):
    server = grpc.aio.server()
    promptforge_pb2_grpc.add_PromptForgeServiceServicer_to_server(PromptForgeServicer(), server)
    server.add_insecure_port(f"unix://{socket_path}")
    await server.start()
    try:
        async with grpc.aio.insecure_channel(f"unix://{socket_path}") as channel:
            stub = promptforge_pb2_grpc.PromptForgeServiceStub(channel)
            await body(stub)
    finally:
        await server.stop(None)


def test_health_responde_pronto(tmp_path):
    async def body(stub):
        resp = await stub.Health(promptforge_pb2.HealthRequest())
        assert resp.ready is True
        assert resp.version == VERSION

    asyncio.run(_with_server(str(tmp_path / "s.sock"), body))


def test_lint_reflete_o_score_do_modulo_puro(tmp_path):
    async def body(stub):
        vago = await stub.Lint(promptforge_pb2.LintRequest(prompt="faça o melhor código rápido"))
        assert vago.score < 0.7
        assert any(i.rule == "vague-term" for i in vago.issues)

        bom = await stub.Lint(
            promptforge_pb2.LintRequest(
                prompt=(
                    "Revise a função de pagamento buscando erros de arredondamento. "
                    "Entrada:\n```python\ndef soma(a, b): return a + b\n```"
                )
            )
        )
        assert bom.score >= 0.9

    asyncio.run(_with_server(str(tmp_path / "s.sock"), body))


def test_list_generators_inclui_code_review(tmp_path):
    async def body(stub):
        resp = await stub.ListGenerators(promptforge_pb2.ListGeneratorsRequest())
        names = {g.name for g in resp.generators}
        assert "code-review" in names
        assert "bug-fix" in names
        code_review = next(g for g in resp.generators if g.name == "code-review")
        assert {f.name for f in code_review.fields} == {"language", "context", "code"}

    asyncio.run(_with_server(str(tmp_path / "s.sock"), body))


def test_render_monta_o_prompt(tmp_path):
    async def body(stub):
        resp = await stub.Render(
            promptforge_pb2.RenderRequest(
                generator="code-review",
                fields={"language": "rust", "context": "gateway", "code": "fn main() {}"},
            )
        )
        assert "rust" in resp.prompt
        assert "fn main() {}" in resp.prompt

    asyncio.run(_with_server(str(tmp_path / "s.sock"), body))


def test_render_gerador_desconhecido_e_not_found(tmp_path):
    async def body(stub):
        with pytest.raises(grpc.aio.AioRpcError) as exc_info:
            await stub.Render(promptforge_pb2.RenderRequest(generator="inexistente", fields={}))
        assert exc_info.value.code() == grpc.StatusCode.NOT_FOUND

    asyncio.run(_with_server(str(tmp_path / "s.sock"), body))


def test_render_campo_obrigatorio_ausente_e_invalid_argument(tmp_path):
    async def body(stub):
        with pytest.raises(grpc.aio.AioRpcError) as exc_info:
            await stub.Render(
                promptforge_pb2.RenderRequest(generator="bug-fix", fields={"symptom": "panica"})
            )
        assert exc_info.value.code() == grpc.StatusCode.INVALID_ARGUMENT

    asyncio.run(_with_server(str(tmp_path / "s.sock"), body))
