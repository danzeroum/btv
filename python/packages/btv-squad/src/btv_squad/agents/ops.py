"""Agente de operações (migrado de BuildToValue `src/agents/ops_agent.py`).

Na origem, `plan_deployment` e `setup_monitoring` eram quase inteiramente
fixos — estágios, health checks, métricas, alertas e configuração de
logging eram constantes, com só a `strategy` variando por uma checagem
simples. Esta versão pede ao gateway um plano de deploy e monitoramento
real informado pela tarefa, numa única chamada — dimensionamento de
scaling/alertas/health-checks vem do modelo, não de números fixos. O
único resquício determinístico é a guarda de domínio (a estratégia
escolhida precisa estar entre `self.deployment_strategies`), mesmo
espírito da guarda de padrão do `DesignerAgent`.
"""

from __future__ import annotations

import json
import logging
from typing import Any

from btv_squad._json import extract_json_object
from btv_squad.agents.base import BaseAgent
from btv_squad.gateway import LlmRequest

logger = logging.getLogger(__name__)

_SYSTEM_PROMPT = """Você é um engenheiro de operações sênior (deploy e observabilidade). Dada uma tarefa, responda SOMENTE com um objeto JSON (sem markdown):
{
  "strategy": "string — estratégia de deploy (blue-green, canary ou rolling)",
  "stages": ["lista de strings — estágios do pipeline de deploy para ESTA tarefa"],
  "rollback_plan": true,
  "health_checks": ["lista de strings — tipos de health check relevantes"],
  "scaling": {"min_instances": 1, "max_instances": 1, "target_cpu": 0},
  "monitoring": {
    "metrics": ["lista de strings"],
    "alerts": [{"metric": "string", "threshold": 0.0, "action": "string"}],
    "dashboards": ["lista de strings"],
    "logging": {"level": "string", "structured": true, "retention_days": 0}
  },
  "confidence": 0.0,
  "notes": "string — decisões operacionais relevantes para ESTA tarefa"
}
Dimensione scaling, health checks e alertas para o serviço descrito — não use valores genéricos que serviriam para qualquer serviço."""

class OpsAgent(BaseAgent):
    """Planeja deploy e monitoramento reais via gateway LLM."""

    def __init__(self, model: str = "claude-sonnet-5") -> None:
        super().__init__("ops")
        self.model = model
        self.deployment_strategies = ["blue-green", "canary", "rolling"]
        self.tools = ["deploy_service", "configure_monitoring"]

    async def execute(self, task: dict[str, Any]) -> dict[str, Any]:
        if not self.validate_input(task):
            raise ValueError("Invalid ops task payload")

        plan = await self.plan_deployment(task)
        decision = {"task": task, "plan": plan, "confidence": plan.get("confidence", 0.0)}
        self.log_decision(decision)
        return {"success": True, "agent": self.agent_type, **plan}

    async def plan_deployment(self, task: dict[str, Any]) -> dict[str, Any]:
        if self.gateway is None:
            raise RuntimeError("OpsAgent sem gateway anexado — chame attach_gateway() antes de execute()")

        request = LlmRequest(
            model=self.model,
            messages=[
                {"role": "system", "content": _SYSTEM_PROMPT},
                {"role": "user", "content": json.dumps(task, ensure_ascii=False)},
            ],
            requester=self.agent_type,
        )
        raw = await self.gateway.generate(request)
        plan = self._parse_plan(raw.text)

        # Guarda de domínio determinística: só aceita uma estratégia suportada.
        if plan["strategy"] not in self.deployment_strategies:
            plan["strategy"] = "blue-green"
        return plan

    def _parse_plan(self, raw_text: str) -> dict[str, Any]:
        parsed = extract_json_object(raw_text)

        return {
            "strategy": parsed.get("strategy", "blue-green"),
            "stages": parsed.get("stages", []),
            "rollback_plan": bool(parsed.get("rollback_plan", False)),
            "health_checks": parsed.get("health_checks", []),
            "scaling": parsed.get("scaling", {}),
            "monitoring": parsed.get("monitoring", {}),
            "confidence": float(parsed.get("confidence", 0.0)),
            "notes": parsed.get("notes", ""),
        }
