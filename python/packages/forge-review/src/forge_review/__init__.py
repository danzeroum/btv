"""Review orientado a valor (origem: BuildToValue `.buildtovalue/review/`).

Quatro reviewers (technical, performance, security, value/ROI) produzem um
`value_score` ponderado; mudanças com score > 0.7 são aprovadas. A migração
completa do orquestrador de review acontece na Fase 5.
"""

from forge_review.score import APPROVAL_THRESHOLD, ReviewScores, value_score

__all__ = ["APPROVAL_THRESHOLD", "ReviewScores", "value_score"]
