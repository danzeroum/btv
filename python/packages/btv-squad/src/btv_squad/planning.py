"""Planejamento adaptativo (migrado de BuildToValue
`src/planning/adaptive_planner.py`).

`planning/hierarchical_planner.py` (`HierarchicalPlanner`) e
`planning/adaptive_replanner.py` (uma **segunda** classe também chamada
`AdaptivePlanner`, com interface totalmente diferente — `execute_with_replanning`,
cujo `_execute_plan` é um placeholder que sempre devolve `{"success": True}`)
NÃO são portados — nenhum dos dois tem chamador real em lugar nenhum do
BuildToValue de origem (confirmado por leitura direta de
`unified_orchestrator.py`, que importa só
`planning.adaptive_planner.AdaptivePlanner`). Mesmo padrão do
`safe_agent_base.py`/`utils/*` descartados no ADR 0004/Onda 1.

Na origem, `_decompose_task` devolvia passos 100% templados (strings
fixas idênticas para qualquer tarefa, só a contagem variando com
`priority == "high"`) e `_calculate_plan_confidence` era uma fórmula
fixa — o mesmo "Nada Fake" já corrigido no `ArchitectAgent.create_plan`
(ADR 0005). Esta versão pede ao gateway a decomposição real da tarefa.

`analyze_failure` continua determinístico (classificação de exceção por
substring) — legítimo, no mesmo espírito de `AuditorAgent.check_security`:
é evidência de entrada, não uma decisão fabricada.
"""

from __future__ import annotations

import json
import logging
import re
import uuid
from datetime import datetime, timezone
from typing import Any

from btv_squad.gateway import LlmRequest

logger = logging.getLogger(__name__)

_JSON_BLOCK = re.compile(r"\{.*\}", re.DOTALL)

_DECOMPOSE_SYSTEM_PROMPT = """Você é um planejador técnico sênior. Dada uma tarefa, decomponha-a em passos executáveis.
Responda SOMENTE com um objeto JSON (sem markdown):
{
  "steps": [
    {"step": 1, "action": "analyze|design|implement|validate|deploy", "description": "string", "estimated_time": 0, "dependencies": [], "can_fail": true}
  ],
  "estimated_duration": 0,
  "confidence": 0.0
}
"action" deve ser um destes 5 valores (mapeiam para os agentes do squad). "dependencies" lista números de "step" que precisam terminar antes. Os passos devem refletir a tarefa específica recebida — nunca repita a mesma decomposição para tarefas diferentes."""

_REPLAN_SYSTEM_PROMPT = """Um passo de um plano falhou. Proponha passos de recuperação.
Responda SOMENTE com um objeto JSON (sem markdown):
{
  "recovery_steps": [
    {"action": "string", "description": "string", "estimated_time": 0, "can_fail": true}
  ],
  "confidence_penalty": 0.0
}
"confidence_penalty" é um float entre 0.0 e 1.0 — quanto reduzir a confiança do plano original dado o tipo de falha. Os passos de recuperação devem refletir o motivo real da falha, não um template genérico de "diagnosticar/corrigir/repetir"."""


class AdaptivePlanner:
    """Cria planos adaptativos reais e recupera de falhas via gateway LLM."""

    def __init__(self, model: str = "claude-sonnet-5") -> None:
        self.model = model
        self.gateway = None  # injetado preguiçosamente (mesmo padrão de BaseAgent)
        self.plan_history: list[dict[str, Any]] = []
        self.failure_patterns: dict[str, int] = {}

    def attach_gateway(self, gateway: Any) -> None:
        self.gateway = gateway

    async def create_adaptive_plan(self, task: dict[str, Any]) -> dict[str, Any]:
        if self.gateway is None:
            raise RuntimeError(
                "AdaptivePlanner sem gateway anexado — chame attach_gateway() antes de create_adaptive_plan()"
            )

        request = LlmRequest(
            model=self.model,
            messages=[
                {"role": "system", "content": _DECOMPOSE_SYSTEM_PROMPT},
                {"role": "user", "content": json.dumps(task, ensure_ascii=False)},
            ],
            requester="planner",
        )
        raw = await self.gateway.generate(request)
        decomposition = self._parse_decomposition(raw.text)

        plan = {
            "plan_id": str(uuid.uuid4()),
            "task_id": task.get("task_id", str(uuid.uuid4())),
            "goal": task.get("description", ""),
            "steps": decomposition["steps"],
            "estimated_duration": decomposition["estimated_duration"],
            "confidence": decomposition["confidence"],
            "created_at": datetime.now(timezone.utc).isoformat(),
            "adaptive": True,
        }
        self.plan_history.append(plan)
        return plan

    async def replan_from_point(
        self, original_plan: dict[str, Any], failed_step: dict[str, Any], reflection: dict[str, Any]
    ) -> dict[str, Any]:
        if self.gateway is None:
            raise RuntimeError(
                "AdaptivePlanner sem gateway anexado — chame attach_gateway() antes de replan_from_point()"
            )

        failure_key = f"{failed_step.get('action')}_{reflection.get('reason', 'unknown')}"
        self.failure_patterns[failure_key] = self.failure_patterns.get(failure_key, 0) + 1

        request = LlmRequest(
            model=self.model,
            messages=[
                {"role": "system", "content": _REPLAN_SYSTEM_PROMPT},
                {
                    "role": "user",
                    "content": json.dumps({"failed_step": failed_step, "reflection": reflection}, ensure_ascii=False),
                },
            ],
            requester="planner",
        )
        raw = await self.gateway.generate(request)
        recovery = self._parse_recovery(raw.text)

        new_plan = dict(original_plan)
        new_plan["plan_id"] = str(uuid.uuid4())
        new_plan["replanned"] = True
        new_plan["replanning_reason"] = reflection
        new_plan["parent_plan"] = original_plan.get("plan_id")

        completed = [step for step in original_plan["steps"] if step["step"] < failed_step["step"]]
        remaining = [step for step in original_plan["steps"] if step["step"] > failed_step["step"]]

        reordered: list[dict[str, Any]] = list(completed)
        next_index = len(reordered) + 1
        for recovery_step in recovery["recovery_steps"]:
            reordered.append(
                {
                    "step": next_index,
                    "action": recovery_step.get("action", failed_step.get("action", "implement")),
                    "description": recovery_step.get("description", ""),
                    "estimated_time": recovery_step.get("estimated_time", 0),
                    "dependencies": [next_index - 1] if next_index > 1 else [],
                    "can_fail": recovery_step.get("can_fail", True),
                    "recovery": True,
                }
            )
            next_index += 1
        for step in remaining:
            step = dict(step)
            step["step"] = next_index
            step["dependencies"] = [next_index - 1] if next_index > 1 else []
            reordered.append(step)
            next_index += 1

        new_plan["steps"] = reordered
        new_plan["confidence"] = max(0.0, original_plan.get("confidence", 0.0) - recovery["confidence_penalty"])
        self.plan_history.append(new_plan)
        return new_plan

    def analyze_failure(self, error: Exception, plan: dict[str, Any]) -> dict[str, Any]:
        """Classificação determinística do tipo de erro — evidência de
        entrada para `replan_from_point`, não uma decisão de plano."""

        message = str(error)
        reason, suggestion = "unknown", "investigate"
        lowered = message.lower()
        if "timeout" in lowered:
            reason, suggestion = "timeout", "increase_timeout"
        elif "memory" in lowered:
            reason, suggestion = "memory_limit", "optimise_memory"
        return {
            "error_type": type(error).__name__,
            "error_message": message,
            "failed_at": datetime.now(timezone.utc).isoformat(),
            "plan_id": plan.get("plan_id"),
            "reason": reason,
            "suggestion": suggestion,
        }

    def _parse_decomposition(self, raw_text: str) -> dict[str, Any]:
        parsed = self._extract_json(raw_text)
        return {
            "steps": parsed.get("steps", []),
            "estimated_duration": int(parsed.get("estimated_duration", 0)),
            "confidence": float(parsed.get("confidence", 0.0)),
        }

    def _parse_recovery(self, raw_text: str) -> dict[str, Any]:
        parsed = self._extract_json(raw_text)
        return {
            "recovery_steps": parsed.get("recovery_steps", []),
            "confidence_penalty": float(parsed.get("confidence_penalty", 0.0)),
        }

    def _extract_json(self, raw_text: str) -> dict[str, Any]:
        match = _JSON_BLOCK.search(raw_text)
        if not match:
            logger.warning("Resposta do modelo não contém um bloco JSON: %r", raw_text[:200])
            return {}
        try:
            candidate = json.loads(match.group(0))
        except json.JSONDecodeError:
            logger.warning("Resposta do modelo não é JSON válido: %r", raw_text[:200])
            return {}
        return candidate if isinstance(candidate, dict) else {}
