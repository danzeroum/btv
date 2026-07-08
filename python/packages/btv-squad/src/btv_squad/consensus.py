"""Consenso ponderado por expertise (migrado de
BuildToValue `src/consensus/weighted_voting.py`, agora tipado com pydantic).

Cada agente tem pesos por domínio de decisão; o voto vale
`peso × confiança`. Consenso abaixo de 0.7 aciona escalonamento humano
(HITL), conforme a metodologia BuildToFlip v6.
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

#: Abaixo desta força de consenso, a decisão escala para humano.
HITL_ESCALATION_THRESHOLD = 0.7


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

    @property
    def requires_human(self) -> bool:
        return self.consensus_strength < HITL_ESCALATION_THRESHOLD


class WeightedConsensusEngine(BaseModel):
    """Agrega propostas com voto ponderado por expertise."""

    agent_weights: dict[str, dict[str, float]] = Field(
        default_factory=lambda: {k: dict(v) for k, v in DEFAULT_AGENT_WEIGHTS.items()}
    )

    def reach_consensus(self, proposals: dict[str, Proposal], decision_type: str) -> ConsensusResult:
        weighted: dict[str, float] = {}
        for agent, proposal in proposals.items():
            weight = self.agent_weights.get(agent, {}).get(decision_type, 0.5)
            weighted[agent] = weight * proposal.confidence

        if not weighted:
            return ConsensusResult(
                decision=None, consensus_strength=0.0, decision_maker=None, dissenting_opinions=[]
            )

        winner = max(weighted, key=weighted.__getitem__)
        total = sum(weighted.values()) or 1.0
        return ConsensusResult(
            decision=proposals[winner],
            consensus_strength=weighted[winner] / total,
            decision_maker=winner,
            dissenting_opinions=[
                Dissent(agent=agent, score=score) for agent, score in weighted.items() if agent != winner
            ],
        )
