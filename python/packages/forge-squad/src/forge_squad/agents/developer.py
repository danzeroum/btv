"""Agente desenvolvedor (migrado de BuildToValue
`src/agents/developer_agent.py`).

Na origem, o "loop ReAct" (`think`/`decide_action`/`execute_action`) era
uma máquina de estados 100% roteirizada — cada "pensamento" e
"observação" era uma string canned escolhida por keyword matching, sem
nenhuma chamada real. Como `CoreService.RunTool` ainda não existe (ativa
só na Onda 4), um loop ReAct de múltiplas iterações executando
ferramentas de verdade não é possível ainda — fingir várias iterações
sem execução real seria trocar uma fabricação por outra. Esta versão
troca a máquina de estados por **uma chamada real** ao gateway que
implementa a tarefa e reporta status/confiança — honesto sobre o escopo
atual, sem fabricar múltiplos passos que não fariam nada de verdade.

`review_system` é injetado como dependência opcional (ADR 0005, decisão
4) — sem ele, `generate_code` devolve o código gerado sem revisão; o
wiring real acontece quando `forge_review` existir (Fase 5).
"""

from __future__ import annotations

import json
import logging
import re
from typing import Any, Optional, Protocol

from forge_squad.agents.base import BaseAgent
from forge_squad.gateway import LlmRequest

logger = logging.getLogger(__name__)

_SYSTEM_PROMPT = """Você é um desenvolvedor full-stack sênior. Dada uma tarefa de implementação, responda SOMENTE com um objeto JSON (sem markdown, sem texto fora do JSON):
{
  "final_output": "string — o código ou artefato implementado para ESTA tarefa",
  "status": "completed ou incomplete",
  "confidence": 0.0,
  "notes": "string — observações relevantes (testes sugeridos, riscos, limitações) para ESTA tarefa"
}
Todos os campos devem refletir a tarefa específica recebida — nunca um placeholder genérico."""

_JSON_BLOCK = re.compile(r"\{.*\}", re.DOTALL)


class ReviewSystem(Protocol):
    """Contrato mínimo do review system (Fase 5, `forge_review`) — só o
    suficiente pro `DeveloperAgent` chamar quando ele existir."""

    async def review_code(self, code: str, metadata: dict[str, Any]) -> dict[str, Any]: ...


class DeveloperAgent(BaseAgent):
    """Desenvolvedor full-stack que implementa tarefas via gateway LLM real."""

    def __init__(self, model: str = "claude-sonnet-5", review_system: Optional[ReviewSystem] = None) -> None:
        super().__init__("developer")
        self.model = model
        self.review_system = review_system
        self.history: list[dict[str, Any]] = []
        self.tools = ["write_code", "generate_tests", "refactor", "debug", "analyze_requirements"]

    async def execute(self, task: dict[str, Any]) -> dict[str, Any]:
        if not self.validate_input(task):
            raise ValueError("Invalid development task payload")

        description = task.get("description", "")
        result = await self.react_loop(description)
        decision = {
            "task": task,
            "result": result,
            "confidence": result.get("confidence", 0.0),
        }
        self.log_decision(decision)
        return {"success": True, "agent": self.agent_type, **result}

    async def create_code(self, task: dict[str, Any]) -> str:
        """Implementa a tarefa via gateway real e devolve o código gerado."""

        description = task.get("description") or task.get("task_description") or ""
        result = await self.react_loop(description)
        return result.get("final_output", "")

    async def generate_code(self, task: dict[str, Any]) -> str:
        """Gera código e, se um `review_system` estiver anexado, roda a
        revisão automática (Fase 5); sem ele, devolve o código sem revisão."""

        code = await self.create_code(task)
        if self.review_system is None:
            return code

        metadata = {
            "task_id": task.get("id") or task.get("task_id"),
            "task_description": task.get("description") or task.get("task_description"),
            "estimated_value": task.get("business_value") or task.get("estimated_value"),
            "priority": task.get("priority", "medium"),
            "filename": task.get("filename", "generated.py"),
        }
        review = await self.review_system.review_code(code=code, metadata=metadata)
        if review.get("approved") and review.get("code"):
            return str(review["code"])
        return await self.auto_fix_issues(code, review.get("reviews", {}))

    async def auto_fix_issues(self, code: str, reviews: dict[str, Any]) -> str:
        """Aplica correções determinísticas para achados comuns de review
        (transformação mecânica sobre um veredito real do review system —
        não é decisão do agente, é bookkeeping sobre a saída dele)."""

        security_review = reviews.get("security", {})
        for vuln in security_review.get("vulnerabilities", []):
            if vuln in code:
                code = code.replace(vuln, f"# Removed insecure usage: {vuln}")

        performance_review = reviews.get("performance", {})
        if performance_review.get("impact") == "Degraded":
            code = "# Optimized placeholder\n" + code

        technical_review = reviews.get("technical", {})
        if technical_review.get("coverage", 0) < 30:
            code += "\n\n# TODO: add tests to increase coverage"

        return code

    async def react_loop(self, task: str) -> dict[str, Any]:
        """Chamada real ao gateway que implementa a tarefa (ver docstring
        do módulo sobre o escopo atual do loop ReAct)."""

        if self.gateway is None:
            raise RuntimeError(
                "DeveloperAgent sem gateway anexado — chame attach_gateway() antes de execute()"
            )

        request = LlmRequest(
            model=self.model,
            messages=[
                {"role": "system", "content": _SYSTEM_PROMPT},
                {"role": "user", "content": task.strip() or "Tarefa não especificada"},
            ],
            requester=self.agent_type,
        )
        raw = await self.gateway.generate(request)
        result = self._parse_result(raw.text)
        self.history.append({"task": task, "result": result})
        return result

    def _parse_result(self, raw_text: str) -> dict[str, Any]:
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
            "final_output": parsed.get("final_output", ""),
            "status": parsed.get("status", "incomplete"),
            "confidence": float(parsed.get("confidence", 0.0)),
            "notes": parsed.get("notes", ""),
        }
