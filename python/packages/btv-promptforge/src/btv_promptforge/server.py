"""Servidor gRPC do PromptForge — primeira ativação do canal Rust↔Python
(Fase 3, ADR 0001/0002). Expõe geradores declarativos e o quality linter
(origem: prompte) sobre um Unix Domain Socket.

Regra de ouro mantida: este serviço nunca chama um provedor de LLM — quem
gera texto de modelo é sempre o gateway Rust (`btv-llm`). Se este sidecar
cair ou não subir, o `btv-sidecar` do lado Rust degrada graciosamente
(pula lint/geradores) em vez de falhar a tarefa do usuário.

Uso: python -m btv_promptforge.server --socket /caminho/para.sock
"""

from __future__ import annotations

import argparse
import asyncio
import logging
import os
from importlib.metadata import PackageNotFoundError, version

import grpc

from btv_proto import promptforge_pb2, promptforge_pb2_grpc
from btv_promptforge.generators import GENERATORS
from btv_promptforge.lint import lint_prompt

logger = logging.getLogger(__name__)

try:
    VERSION = version("btv-promptforge")
except PackageNotFoundError:  # pragma: no cover - fora de um ambiente instalado
    VERSION = "0.1.0"


class PromptForgeServicer(promptforge_pb2_grpc.PromptForgeServiceServicer):
    """Implementação do `PromptForgeService` sobre os módulos puros existentes."""

    async def Health(self, request, context):  # noqa: N802 (nome do protobuf)
        return promptforge_pb2.HealthResponse(ready=True, version=VERSION)

    async def Lint(self, request, context):  # noqa: N802
        report = lint_prompt(request.prompt)
        return promptforge_pb2.LintReport(
            score=report.score,
            grade=report.grade,
            issues=[
                promptforge_pb2.LintIssue(rule=issue.rule, message=issue.message)
                for issue in report.issues
            ],
        )

    async def Render(self, request, context):  # noqa: N802
        generator = GENERATORS.get(request.generator)
        if generator is None:
            await context.abort(
                grpc.StatusCode.NOT_FOUND, f"gerador desconhecido: {request.generator}"
            )
            return
        try:
            prompt = generator.render(dict(request.fields))
        except ValueError as exc:
            await context.abort(grpc.StatusCode.INVALID_ARGUMENT, str(exc))
            return
        return promptforge_pb2.RenderResponse(prompt=prompt)

    async def ListGenerators(self, request, context):  # noqa: N802
        infos = [
            promptforge_pb2.GeneratorInfo(
                name=g.name,
                category=g.category,
                fields=[
                    promptforge_pb2.GeneratorField(
                        name=f.name,
                        label=f.label,
                        required=f.required,
                        placeholder=f.placeholder,
                    )
                    for f in g.fields
                ],
            )
            for g in GENERATORS.values()
        ]
        return promptforge_pb2.ListGeneratorsResponse(generators=infos)


async def serve(socket_path: str) -> None:
    if os.path.exists(socket_path):
        os.remove(socket_path)
    server = grpc.aio.server()
    promptforge_pb2_grpc.add_PromptForgeServiceServicer_to_server(PromptForgeServicer(), server)
    server.add_insecure_port(f"unix://{socket_path}")
    await server.start()
    logger.info("btv_promptforge sidecar ouvindo em %s", socket_path)
    await server.wait_for_termination()


def main() -> None:
    parser = argparse.ArgumentParser(description="Sidecar gRPC do PromptForge")
    parser.add_argument("--socket", required=True, help="caminho do Unix Domain Socket")
    args = parser.parse_args()
    logging.basicConfig(level=os.environ.get("BTV_LOG_LEVEL", "INFO").upper())
    asyncio.run(serve(args.socket))


if __name__ == "__main__":
    main()
