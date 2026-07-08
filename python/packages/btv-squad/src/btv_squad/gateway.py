"""Contrato do gateway LLM consumido pelos agentes do squad — desacoplado
do transporte gRPC real (`CoreService.Generate`,
`schemas/proto/{core,llm}.proto`) para que a Onda 2 (agentes reais) não
fique bloqueada pela Onda 4 (ativação do `SquadService`/`CoreService`
sobre UDS). Ver ADR 0005.

Regra de ouro (ADR 0001) preservada: a implementação real deste Protocol
(Onda 4) fala com o núcleo Rust por gRPC — nunca com um provider LLM
diretamente.
"""

from __future__ import annotations

from typing import Protocol

from pydantic import BaseModel, Field


class LlmRequest(BaseModel):
    """Espelha `btv.llm.v1.LlmRequest` (`schemas/proto/llm.proto`)."""

    model: str
    messages: list[dict] = Field(default_factory=list)
    temperature: float | None = None
    max_tokens: int | None = None
    #: Nome do agente solicitante — telemetria e rate limiting no lado Rust.
    requester: str


class LlmResponse(BaseModel):
    """Agregado dos `LlmChunk` (`text_delta`/`usage`) de um stream."""

    text: str
    input_tokens: int = 0
    output_tokens: int = 0
    cache_hit: bool = False
    provider: str = ""


class GatewayClient(Protocol):
    """Contrato consumido pelos agentes. A implementação real (Onda 4)
    fala gRPC com `CoreService.Generate`; testes usam
    `ScriptedGatewayClient`.
    """

    async def generate(self, request: LlmRequest) -> LlmResponse: ...


class ScriptedGatewayClient:
    """Gateway falso e determinístico para testes — mesmo princípio do
    gerador roteirizado usado nos testes Rust do loop de agente
    (`btv-core`). Levanta se um agente pedir mais chamadas do que o
    teste programou, em vez de devolver uma resposta genérica.
    """

    def __init__(self, responses: list[LlmResponse]) -> None:
        self._responses = list(responses)
        self.requests: list[LlmRequest] = []

    async def generate(self, request: LlmRequest) -> LlmResponse:
        self.requests.append(request)
        if not self._responses:
            raise AssertionError("ScriptedGatewayClient esgotou as respostas roteirizadas")
        return self._responses.pop(0)
