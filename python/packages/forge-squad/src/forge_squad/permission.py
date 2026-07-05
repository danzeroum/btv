"""Contrato do client de permissão/HITL consumido pelo
`ProgressiveAutonomyManager` — desacoplado do transporte gRPC real
(`CoreService.RequestPermission`, `schemas/proto/core.proto`) pelo mesmo
motivo do `GatewayClient` (ADR 0005): `core.proto` ainda não tem stubs
gerados (só `promptforge.proto` compila hoje), e a decisão chega à TUI
só na Onda 4. Sem esta fronteira, o HITL da Onda 3 ficaria bloqueado
esperando a Onda 4 — na ordem errada.
"""

from __future__ import annotations

from typing import Protocol

from pydantic import BaseModel


class PermissionRequest(BaseModel):
    """Espelha `forge.core.v1.PermissionRequest` (`schemas/proto/core.proto`)."""

    tool: str
    scope: str
    reason: str
    #: Confiança do agente na ação — mesmo campo do proto (gatilho HITL: < 0.3/0.5).
    confidence: float


class PermissionDecision(BaseModel):
    """Espelha `forge.core.v1.PermissionDecision`."""

    approved: bool
    operator_note: str | None = None


class PermissionClient(Protocol):
    """Contrato consumido pelo `ProgressiveAutonomyManager`. A implementação
    real (Onda 4) fala gRPC com `CoreService.RequestPermission`, que a TUI
    resolve com um modal (s/n) — mesmo padrão do `PermissionResolver` do
    `forge-core` em Rust. Testes usam `ScriptedPermissionClient`.
    """

    async def request_permission(self, request: PermissionRequest) -> PermissionDecision: ...


class ScriptedPermissionClient:
    """Client de permissão falso e determinístico para testes — mesmo
    princípio do `ScriptedGatewayClient`.
    """

    def __init__(self, decisions: list[PermissionDecision]) -> None:
        self._decisions = list(decisions)
        self.requests: list[PermissionRequest] = []

    async def request_permission(self, request: PermissionRequest) -> PermissionDecision:
        self.requests.append(request)
        if not self._decisions:
            raise AssertionError("ScriptedPermissionClient esgotou as decisões roteirizadas")
        return self._decisions.pop(0)
