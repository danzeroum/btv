"""Consenso ponderado por expertise (migrado de
BuildToValue `src/consensus/weighted_voting.py`, agora tipado com pydantic).

Cada agente tem pesos por domínio de decisão; o voto vale
`peso × confiança`. Consenso abaixo do limiar escalona para humano (HITL).

Semântica do número exposto (PATCH análise do ciclo completo): o número
visível ao usuário (`consensus_strength`) é a **participação do vencedor no
total ponderado** (`winner_share`), NÃO a confiança média nem a confiança do
vencedor — com 3 agentes e pesos 0.9/0.6/0.5 o teto estrutural é 45%, então
o limiar fixo de 0.7 era inalcançável por construção e o HITL sempre
disparava. Agora o limiar é **calibrado** contra esse teto
(`max(0.3, min(0.7, max_possible × 0.85))`), e o evento carrega os dois
números com o `metric_definition` explícito ("winner_share") — nada de
fabricar precisão.
"""

from __future__ import annotations

from pydantic import BaseModel, Field

DEFAULT_AGENT_WEIGHTS: dict[str, dict[str, float]] = {
    "architect": {"architecture": 0.9, "security": 0.7},
    "developer": {"architecture": 0.6, "implementation": 0.95, "testing": 0.8},
    "auditor": {"security": 0.95, "quality": 0.85},
    "designer": {"ui": 0.95, "ux": 0.9},
    "ops": {"infrastructure": 0.9, "deployment": 0.9},
}

#: Teto do limiar calibrado — nunca pedimos consenso mais forte que isto.
HITL_ESCALATION_THRESHOLD = 0.7

#: Piso do limiar calibrado — uma maioria ponderada mínima ainda escala.
HITL_CALIBRATION_FLOOR = 0.3

#: Fator do teto estrutural usado para calibrar o limiar por decisão.
CALIBRATION_FACTOR = 0.85

#: Confiança mínima assumida dos votos perdedores no cálculo do teto
#: estrutural (pior caso: todos os outros no piso de confiança).
_MIN_CONFIDENCE_FALLBACK = 0.5

#: O número exposto é sempre esta métrica — nada de ler como "confiança".
METRIC_DEFINITION = "winner_share"


def _calibrated_threshold(max_possible: float) -> float:
    """Limiar escalável por estrutura de pesos: entre `HITL_CALIBRATION_FLOOR`
    e `HITL_ESCALATION_THRESHOLD`, proporcional ao teto estrutural."""
    return max(HITL_CALIBRATION_FLOOR, min(HITL_ESCALATION_THRESHOLD, max_possible * CALIBRATION_FACTOR))


class Proposal(BaseModel):
    """Proposta de um agente para uma decisão."""

    confidence: float = Field(default=0.5, ge=0.0, le=1.0)
    content: dict = Field(default_factory=dict)


class Dissent(BaseModel):
    agent: str
    score: float


class ConsensusResult(BaseModel):
    decision: Proposal | None
    consensus_strength: float
    decision_maker: str | None
    dissenting_opinions: list[Dissent]
    #: Confiança REAL do vencedor (não a participação ponderada).
    winner_confidence: float = 0.0
    #: Limiar efetivamente aplicado a esta decisão (calibrado pelo teto).
    threshold_applied: float = HITL_ESCALATION_THRESHOLD
    #: Rótulo explícito do número exposto — sempre "winner_share".
    metric_definition: str = METRIC_DEFINITION
    #: Confianças individuais por agente (para exibição honesta).
    proposal_confidences: dict[str, float] = Field(default_factory=dict)
    #: Pesos efetivamente usados nesta decisão (por agente).
    agent_weights: dict[str, float] = Field(default_factory=dict)
    #: Veredito do auditor quando ele propôs: True/False, None se ausente.
    auditor_verdict: bool | None = None

    @property
    def requires_human(self) -> bool:
        """Escala para humano por consenso fraco OU veto explícito do
        auditor — a reprovação do auditor nunca é atropelada por uma média
        alta (fail-closed, mesmo espírito do `gates.evaluate`)."""
        if self.auditor_verdict is False:
            return True
        return self.consensus_strength < self.threshold_applied


class WeightedConsensusEngine(BaseModel):
    """Agrega propostas com voto ponderado por expertise."""

    agent_weights: dict[str, dict[str, float]] = Field(
        default_factory=lambda: {k: dict(v) for k, v in DEFAULT_AGENT_WEIGHTS.items()}
    )

    def _weights_for(self, proposals: dict[str, Proposal], decision_type: str) -> dict[str, float]:
        return {
            agent: self.agent_weights.get(agent, {}).get(decision_type, 0.5) for agent in proposals
        }

    def _max_possible(self, weights: dict[str, float]) -> float:
        """Teto estrutural: melhor participação que o agente mais forte
        PODERIA atingir nesta composição (ele com confiança 1.0, os demais
        no piso 0.5). Independe das confianças reais — é propriedade dos
        pesos e do tamanho do squad; exposto via `threshold_applied`."""
        if not weights:
            return 0.0
        best = 0.0
        for winner, w in weights.items():
            others = sum(weight * _MIN_CONFIDENCE_FALLBACK for agent, weight in weights.items() if agent != winner)
            share = w / (w + others) if (w + others) else 0.0
            best = max(best, share)
        return best

    def _auditor_verdict(self, proposals: dict[str, Proposal]) -> bool | None:
        """Veto do auditor: sua proposta carrega o veredito em `content`
        (o dict real de `AuditorAgent.execute`, com `assessment` aninhado
        ou `approved`/`passed` no topo, conforme o chamador) — `False` em
        qualquer um = reprovação explícita, nunca atropelada por média alta."""
        auditor = proposals.get("auditor")
        if auditor is None:
            return None
        content = auditor.content if isinstance(auditor.content, dict) else {}
        assessment = content.get("assessment")
        if isinstance(assessment, dict):
            content = assessment
        verdict = content.get("approved", content.get("passed"))
        return verdict if isinstance(verdict, bool) else None

    def reach_consensus(self, proposals: dict[str, Proposal], decision_type: str) -> ConsensusResult:
        weights = self._weights_for(proposals, decision_type)
        weighted: dict[str, float] = {}
        for agent, proposal in proposals.items():
            weighted[agent] = weights[agent] * proposal.confidence

        if not weighted:
            return ConsensusResult(
                decision=None,
                consensus_strength=0.0,
                decision_maker=None,
                dissenting_opinions=[],
                proposal_confidences={},
                agent_weights={},
                threshold_applied=_calibrated_threshold(0.0),
            )

        winner = max(weighted, key=weighted.__getitem__)
        total = sum(weighted.values()) or 1.0
        max_possible = self._max_possible(weights)
        return ConsensusResult(
            decision=proposals[winner],
            consensus_strength=weighted[winner] / total,
            decision_maker=winner,
            dissenting_opinions=[
                Dissent(agent=agent, score=score) for agent, score in weighted.items() if agent != winner
            ],
            winner_confidence=proposals[winner].confidence,
            threshold_applied=_calibrated_threshold(max_possible),
            proposal_confidences={agent: p.confidence for agent, p in proposals.items()},
            agent_weights=weights,
            auditor_verdict=self._auditor_verdict(proposals),
        )