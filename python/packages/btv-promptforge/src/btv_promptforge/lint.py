"""Quality linter de prompts — "ESLint para prompts" (origem: prompte
`promptQuality.js`): detecta termos vagos, checa presença de contexto e
entrada concreta, devolve score com sugestões.
"""

from __future__ import annotations

from pydantic import BaseModel

VAGUE_TERMS = [
    "melhor",
    "bom",
    "rápido",
    "simples",
    "otimizado",
    "adequado",
    "apropriado",
    "etc",
]

MIN_CONTEXT_LENGTH = 40


class LintIssue(BaseModel):
    rule: str
    message: str


class LintReport(BaseModel):
    score: float  # 0.0–1.0
    issues: list[LintIssue]

    @property
    def grade(self) -> str:
        if self.score >= 0.9:
            return "A"
        if self.score >= 0.7:
            return "B"
        if self.score >= 0.5:
            return "C"
        return "D"


def lint_prompt(prompt: str) -> LintReport:
    issues: list[LintIssue] = []
    lowered = prompt.lower()

    found_vague = [term for term in VAGUE_TERMS if f" {term}" in f" {lowered}"]
    for term in found_vague:
        issues.append(
            LintIssue(
                rule="vague-term",
                message=f"termo vago '{term}': especifique o critério concreto",
            )
        )

    if len(prompt.strip()) < MIN_CONTEXT_LENGTH:
        issues.append(
            LintIssue(rule="missing-context", message="prompt curto demais: adicione contexto do projeto e objetivo")
        )

    if "```" not in prompt and not any(marker in lowered for marker in ("entrada:", "input:", "exemplo:")):
        issues.append(
            LintIssue(rule="missing-input", message="nenhuma entrada concreta (código, exemplo ou dados) encontrada")
        )

    penalty = 0.2 * len(issues)
    return LintReport(score=max(0.0, 1.0 - penalty), issues=issues)
