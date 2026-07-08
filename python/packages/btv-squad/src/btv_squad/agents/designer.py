"""Agente designer (migrado de BuildToValue `src/agents/designer_agent.py`).

Na origem, `create_design` era quase inteiramente fixo — cores, tipografia,
padrão de acessibilidade e confiança (0.85) eram constantes, com só o
padrão de design e o componente "hero" variando por uma checagem simples.
Esta versão pede ao gateway um design real informado pela tarefa; o único
resquício determinístico é uma guarda de domínio legítima (o padrão
escolhido precisa estar entre os suportados por `self.design_patterns`,
mesmo espírito do design-system real) — não é fabricação, é validação de
uma escolha externa contra uma lista de opções válidas.
"""

from __future__ import annotations

import json
import logging
import re
from typing import Any

from btv_squad.agents.base import BaseAgent
from btv_squad.gateway import LlmRequest

logger = logging.getLogger(__name__)

_SYSTEM_PROMPT = """Você é um designer de produto sênior (UX/UI). Dada uma tarefa de design, responda SOMENTE com um objeto JSON (sem markdown):
{
  "pattern": "string — sistema de design recomendado (material, fluent ou carbon)",
  "components": ["lista de strings — componentes de UI necessários para ESTA tarefa"],
  "colors": {"primary": "#hex", "secondary": "#hex", "background": "#hex"},
  "typography": {"font": "string", "sizes": {"h1": "string", "body": "string"}},
  "responsive": true,
  "accessibility": "string — padrão de acessibilidade visado (ex.: WCAG 2.1 AA)",
  "confidence": 0.0,
  "notes": "string — decisões de design relevantes para ESTA tarefa"
}
Todos os campos devem refletir a tarefa específica — não repita um design genérico."""

_JSON_BLOCK = re.compile(r"\{.*\}", re.DOTALL)


class DesignerAgent(BaseAgent):
    """Produz artefatos de design reais via gateway LLM."""

    def __init__(self, model: str = "claude-sonnet-5") -> None:
        super().__init__("designer")
        self.model = model
        self.design_patterns: list[str] = ["material", "fluent", "carbon"]
        self.tools = ["design_mock", "accessibility_review"]

    async def execute(self, task: dict[str, Any]) -> dict[str, Any]:
        if not self.validate_input(task):
            raise ValueError("Invalid design task payload")

        design = await self.create_design(task)
        decision = {"task": task, "design": design, "confidence": design.get("confidence", 0.0)}
        self.log_decision(decision)
        return {"success": True, "agent": self.agent_type, **design}

    async def create_design(self, task: dict[str, Any]) -> dict[str, Any]:
        if self.gateway is None:
            raise RuntimeError(
                "DesignerAgent sem gateway anexado — chame attach_gateway() antes de execute()"
            )

        request = LlmRequest(
            model=self.model,
            messages=[
                {"role": "system", "content": _SYSTEM_PROMPT},
                {"role": "user", "content": json.dumps(task, ensure_ascii=False)},
            ],
            requester=self.agent_type,
        )
        raw = await self.gateway.generate(request)
        design = self._parse_design(raw.text)

        # Guarda de domínio determinística: só aceita um padrão suportado.
        if design["pattern"] not in self.design_patterns:
            design["pattern"] = "material"
        return design

    def _parse_design(self, raw_text: str) -> dict[str, Any]:
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
            "pattern": parsed.get("pattern", "material"),
            "components": parsed.get("components", []),
            "colors": parsed.get("colors", {}),
            "typography": parsed.get("typography", {}),
            "responsive": bool(parsed.get("responsive", False)),
            "accessibility": parsed.get("accessibility", ""),
            "confidence": float(parsed.get("confidence", 0.0)),
            "notes": parsed.get("notes", ""),
        }
