"""Avaliação contínua da qualidade dos agentes (migrado de BuildToValue
`src/evaluation/continuous_evaluator.py` — o canônico, importado pelo
`UnifiedOrchestrator`; `continuous_eval.py` era a lineage descartada,
ADR 0004).

Na origem, `evaluate_technical_quality` devolvia
`result.get("technical_score", 0.8)` — mas nenhum agente real produz um
campo `technical_score`, então o valor era **sempre** o default 0.8, e o
portão de replanejamento do orquestrador (`technical_score < 0.6`) nunca
disparava. Mesmo "Nada Fake" já corrigido no `create_plan`/`_decompose_task`:
um default fabricado escondido atrás de um `.get()`. Esta versão deriva a
qualidade técnica dos campos reais que os agentes de fato reportam
(`confidence` + `success`, ambos vindos de chamadas reais ao gateway), e
calcula `improvement` como delta contra a média histórica real — não um
default. `business_score` foi removido: não há sinal de valor de negócio
no resultado do agente para derivar, e fabricar 0.7 seria o mesmo bug.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class ContinuousEvaluator:
    """Coleta métricas rolantes e pontua a qualidade real dos agentes."""

    metrics: dict[str, list[float]] = field(
        default_factory=lambda: {
            "task_success_rate": [],
            "average_confidence": [],
            "technical_score": [],
        }
    )

    async def evaluate_agent_performance(self, agent_name: str, task_result: dict[str, Any]) -> dict[str, Any]:
        technical = self.evaluate_technical_quality(task_result)
        improvement = self.compare_with_baseline(technical)

        self._record("task_success_rate", 1.0 if task_result.get("success") else 0.0)
        self._record("average_confidence", float(task_result.get("confidence", 0.0)))
        self._record("technical_score", technical)

        return {
            "agent": agent_name,
            "technical_score": technical,
            "improvement": improvement,
        }

    def evaluate_technical_quality(self, result: dict[str, Any]) -> float:
        """Qualidade técnica derivada do resultado real do agente — a
        confiança que ele reportou (via gateway), zerada se a execução
        falhou. Nenhum default fabricado."""

        if not result.get("success", False):
            return 0.0
        return float(result.get("confidence", 0.0))

    def compare_with_baseline(self, technical: float) -> float:
        """Melhoria = quanto este score supera a média histórica real de
        `technical_score` (0.0 na primeira avaliação, sem baseline)."""

        history = self.metrics["technical_score"]
        if not history:
            return 0.0
        baseline = sum(history) / len(history)
        return technical - baseline

    def _record(self, metric: str, value: float) -> None:
        if metric in self.metrics:
            self.metrics[metric].append(value)
