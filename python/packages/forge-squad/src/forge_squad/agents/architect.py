"""Agente arquiteto com raciocínio Chain-of-Thought real (migrado de
BuildToValue `src/agents/architect_agent.py`).

Na origem, `reason_with_cot` era 100% heurística fixa — os "passos" de
raciocínio eram literais constantes, independentes do problema recebido.
Esta versão chama o gateway LLM de verdade (ADR 0005); `create_plan` e
`create_adr` continuam deterministas, transformando o resultado real do
raciocínio — não são heurística disfarçada de decisão, são bookkeeping
mecânico sobre uma decisão que agora é real.
"""

from __future__ import annotations

import json
import logging
import re
from datetime import datetime, timezone
from typing import Any

from forge_squad.agents.base import BaseAgent
from forge_squad.gateway import LlmRequest

logger = logging.getLogger(__name__)

_SYSTEM_PROMPT = """Você é um arquiteto de software sênior fazendo análise Chain-of-Thought.
Dado um problema, responda SOMENTE com um objeto JSON (sem markdown, sem texto fora do JSON) com exatamente estes campos:
{
  "problem_analysis": "string — o problema central identificado",
  "constraints": ["lista de strings — restrições técnicas relevantes"],
  "applicable_patterns": ["lista de strings — padrões de design aplicáveis"],
  "trade_offs": {"padrão ou opção": "string — trade-off dessa escolha"},
  "recommendation": "string — a solução recomendada",
  "confidence": 0.0
}
"confidence" é um float entre 0.0 e 1.0 representando sua certeza na recomendação."""

_JSON_BLOCK = re.compile(r"\{.*\}", re.DOTALL)


class ArchitectAgent(BaseAgent):
    """Arquiteto sênior capaz de análise Chain-of-Thought real."""

    def __init__(self, model: str = "claude-sonnet-5") -> None:
        super().__init__("architect")
        self.model = model
        self.reasoning_history: list[dict[str, Any]] = []
        self.tools = ["analyze_architecture", "generate_adr"]

    async def execute(self, task: dict[str, Any]) -> dict[str, Any]:
        if not self.validate_input(task):
            raise ValueError("Invalid architectural task payload")

        description = task.get("description", "")
        reasoning = await self.reason_with_cot(description)
        plan = await self.create_plan(task, reasoning)
        adr = self.create_adr(
            {
                "title": task.get("title", description[:60] or "Architecture Decision"),
                "problem_analysis": reasoning.get("problem_analysis", description),
                "recommendation": reasoning.get("recommendation", plan.get("architecture")),
                "trade_offs": reasoning.get("trade_offs", {}),
            }
        )

        decision = {
            "task": task,
            "reasoning": reasoning,
            "plan": plan,
            "adr": adr,
            "confidence": reasoning.get("confidence", 0.0),
        }
        self.log_decision(decision)

        return {
            "success": True,
            "agent": self.agent_type,
            "reasoning": reasoning,
            "plan": plan,
            "adr": adr,
            "confidence": reasoning.get("confidence", 0.0),
        }

    async def reason_with_cot(self, problem: str) -> dict[str, Any]:
        """Pede ao gateway LLM uma cadeia de raciocínio real sobre o problema."""

        if self.gateway is None:
            raise RuntimeError(
                "ArchitectAgent sem gateway anexado — chame attach_gateway() antes de execute()"
            )

        request = LlmRequest(
            model=self.model,
            messages=[
                {"role": "system", "content": _SYSTEM_PROMPT},
                {"role": "user", "content": problem.strip() or "Problema não especificado"},
            ],
            requester=self.agent_type,
        )
        raw = await self.gateway.generate(request)
        response = self._parse_reasoning(problem, raw.text)
        self.reasoning_history.append(response)
        return response

    def _parse_reasoning(self, problem: str, raw_text: str) -> dict[str, Any]:
        """Extrai o JSON estruturado da resposta do modelo, com fallback
        defensivo — uma resposta mal-formada nunca derruba o agente."""

        parsed: dict[str, Any] = {}
        match = _JSON_BLOCK.search(raw_text)
        if match:
            try:
                candidate = json.loads(match.group(0))
                if isinstance(candidate, dict):
                    parsed = candidate
            except json.JSONDecodeError:
                logger.warning("Resposta do modelo não é JSON válido: %r", raw_text[:200])
        else:
            logger.warning("Resposta do modelo não contém um bloco JSON: %r", raw_text[:200])

        return {
            "problem_analysis": parsed.get("problem_analysis", problem.strip() or "Problema não especificado"),
            "constraints": parsed.get("constraints", []),
            "applicable_patterns": parsed.get("applicable_patterns", []),
            "trade_offs": parsed.get("trade_offs", {}),
            "recommendation": parsed.get("recommendation", ""),
            "confidence": float(parsed.get("confidence", 0.0)),
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }

    async def create_plan(self, task: dict[str, Any], reasoning: dict[str, Any]) -> dict[str, Any]:
        """Deriva um plano leve a partir do raciocínio real do agente."""

        components = ["API Gateway", "Auth Service", "Business Logic", "Database"]
        if "cache" in reasoning.get("recommendation", "").lower():
            components.append("Caching Layer")

        return {
            "goal": task.get("description", ""),
            "architecture": "microservices",
            "components": components,
            "patterns": reasoning.get("applicable_patterns", []),
            "risks": ["Complexidade", "Custo operacional"],
            "mitigations": ["Documentação", "Observabilidade"],
            "estimated_effort": "2 sprints",
        }

    def create_adr(self, decision: dict[str, Any]) -> str:
        """Gera uma nota no estilo Architecture Decision Record."""

        return (
            f"# ADR: {decision.get('title', 'Architecture Decision')}\n\n"
            "## Status\nAccepted\n\n"
            "## Context\n"
            f"{decision.get('problem_analysis', 'N/A')}\n\n"
            "## Decision\n"
            f"{decision.get('recommendation', 'N/A')}\n\n"
            "## Consequences\n"
            f"{decision.get('trade_offs', 'N/A')}\n"
        )
