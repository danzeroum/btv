"""Cálculo do value_score ponderado (contrato do BuildToValue review system)."""

from __future__ import annotations

from pydantic import BaseModel, Field

APPROVAL_THRESHOLD = 0.7

WEIGHTS = {
    "technical": 0.25,
    "performance": 0.20,
    "security": 0.30,
    "value": 0.25,
}


class ReviewScores(BaseModel):
    technical: float = Field(ge=0.0, le=1.0)
    performance: float = Field(ge=0.0, le=1.0)
    security: float = Field(ge=0.0, le=1.0)
    value: float = Field(ge=0.0, le=1.0)


def value_score(scores: ReviewScores) -> float:
    """Média ponderada das quatro dimensões de review."""
    return sum(getattr(scores, dim) * weight for dim, weight in WEIGHTS.items())


def is_approved(scores: ReviewScores) -> bool:
    return value_score(scores) > APPROVAL_THRESHOLD
