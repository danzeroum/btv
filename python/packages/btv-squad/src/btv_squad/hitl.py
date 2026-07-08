"""Autonomia progressiva / human-in-the-loop (migrado de BuildToValue
`src/hitl/progressive_autonomy.py`).

Dois placeholders na origem escondiam fabricação atrás de bookkeeping
real: `_request_human_approval` sempre devolvia `{"approved": True}` (um
carimbo automático — o mesmo risco já tratado no `AuditorAgent`), e
`_execute_action` sempre devolvia `{"success": True}`. O segundo é
particularmente enganoso porque o único chamador real
(`UnifiedOrchestrator.execute_complex_task`) **nunca lê o resultado dessa
execução fake** — só verifica `approval.get("executed")` como um portão
booleano; a execução de verdade acontece depois, em
`_execute_plan_steps`. Ou seja, `_execute_action` era fabricação morta:
nem sequer alimentava uma decisão real.

Esta versão trata `execute_with_autonomy` como **só o portão** (pede
aprovação humana real via `PermissionClient` quando o nível de autonomia
exige, mesmo padrão do `GatewayClient`/ADR 0005 — `core.proto` também não
tem stubs gerados ainda) e não finge executar a ação. `record_action`
fica público para quem de fato executa a ação (o orquestrador, Onda 4)
chamar depois, com o resultado real — separando "decidir se pode" de
"fazer", mesmo espírito do `PermissionResolver` do `btv-core` em Rust
(decide sim/não; quem executa a ferramenta é outra camada).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Optional

from btv_squad.permission import PermissionClient, PermissionRequest


@dataclass
class ProgressiveAutonomyManager:
    """Acompanha score de confiança e nível de autonomia por agente."""

    autonomy_levels: dict[int, str] = field(
        default_factory=lambda: {
            0: "full_human_control",
            1: "human_approval_critical",
            2: "human_notification",
            3: "full_autonomy",
        }
    )
    agent_trust_scores: dict[str, float] = field(default_factory=dict)
    action_history: list[dict[str, Any]] = field(default_factory=list)
    permission_client: Optional[PermissionClient] = None

    def attach_permission_client(self, client: PermissionClient) -> None:
        self.permission_client = client

    def _get_autonomy_level(self, agent: str) -> int:
        score = self.agent_trust_scores.get(agent, 0.5)
        if score < 0.4:
            return 0
        if score < 0.6:
            return 1
        if score < 0.8:
            return 2
        return 3

    async def execute_with_autonomy(self, agent: str, action: dict[str, Any]) -> dict[str, Any]:
        """Portão de aprovação — não executa a ação, só decide se ela pode
        prosseguir. Quem chama é responsável por executar de verdade e
        depois reportar o resultado via `record_action`."""

        critical = action.get("critical", False)
        if not self._needs_human_approval(agent, critical):
            return {"executed": True}

        if self.permission_client is None:
            raise RuntimeError(
                "ProgressiveAutonomyManager sem permission_client anexado — "
                "chame attach_permission_client() antes de execute_with_autonomy() "
                "quando aprovação humana for necessária"
            )

        request = PermissionRequest(
            tool=str(action.get("action", "unknown")),
            scope=agent,
            reason=str(action.get("plan", action)),
            confidence=self.agent_trust_scores.get(agent, 0.5),
        )
        decision = await self.permission_client.request_permission(request)
        if not decision.approved:
            self.record_action(agent, action, success=False)
            return {"executed": False, "reason": "Rejected by human", "feedback": decision.operator_note}
        return {"executed": True}

    def record_action(self, agent: str, action: dict[str, Any], success: bool) -> None:
        """Registra o resultado real de uma ação e ajusta o score de
        confiança — chamado após a execução de verdade (ou após rejeição
        humana), nunca com um resultado fabricado."""

        current_level = self._get_autonomy_level(agent)
        self._update_score(agent, success)
        self.action_history.append(
            {
                "timestamp": datetime.now(timezone.utc).isoformat(),
                "agent": agent,
                "action": action,
                "success": success,
                "trust_score": self.agent_trust_scores[agent],
                "autonomy_level": current_level,
            }
        )

    def _update_score(self, agent: str, success: bool) -> float:
        score = self.agent_trust_scores.get(agent, 0.5)
        if success:
            score = min(1.0, score + 0.02)
        else:
            score = max(0.0, score - 0.1)
        self.agent_trust_scores[agent] = score
        return score

    def _needs_human_approval(self, agent: str, critical: bool) -> bool:
        level = self._get_autonomy_level(agent)
        if level == 3:
            return False
        if level == 2:
            return critical
        return True

    def needs_human_approval(self, agent: str, critical: bool) -> bool:
        return self._needs_human_approval(agent, critical)
