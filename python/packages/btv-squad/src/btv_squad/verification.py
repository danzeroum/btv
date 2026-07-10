"""Espelho Pydantic do ``verification-evidence.v1`` (D3t).

Antes a evidência viajava como string JSON no
``SquadTask.verification_evidence_json`` e o ``server.py`` fazia
``json.loads`` → ``dict[str, Any]`` — sem tipo, validado à mão com
``isinstance``. O D3t tipa o campo no wire (mensagem proto tipada); este
módulo dá a ela UM tipo do lado Python — parse-don't-validate, como o
``TenantContext`` do D4t.

``to_wire_dict()`` reproduz o JSON canônico do schema — mesma forma que o
Rust emite (``btv_schemas::verification`` com ``skip_serializing_if =
Option::is_none`` em ``file``/``line``): paridade com o caminho antigo, para
o prompt do auditor ficar byte-idêntico. O ``dict`` que desce para o
orquestrador é a evidência canônica (dado para o LLM pesar), não mais o
resultado de um ``json.loads`` opaco.
"""

from __future__ import annotations

from typing import Any, Optional

from pydantic import BaseModel

#: Mapeia o enum ``Verdict`` do proto → string do schema. ``0``
#: (``VERDICT_UNSPECIFIED``, zero-value proto3) NÃO tem forma no schema
#: (["pass","fail","skipped"]) — vira ``None`` e a evidência é recusada
#: fail-closed (o Rust nunca emite UNSPECIFIED; só um caller quebrado o faria).
_VERDICT_FROM_PROTO = {0: None, 1: "pass", 2: "fail", 3: "skipped"}


class Finding(BaseModel):
    """Achado de uma ferramenta num passo de verificação."""

    tool: str
    severity: str
    message: str
    file: Optional[str] = None
    line: Optional[int] = None

    @classmethod
    def from_proto(cls, msg: Any) -> "Finding":
        return cls(
            tool=msg.tool,
            severity=msg.severity,
            message=msg.message,
            # proto3 `optional` → presença explícita (ausência ≠ vazio).
            file=msg.file if msg.HasField("file") else None,
            line=msg.line if msg.HasField("line") else None,
        )

    def to_wire_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {
            "tool": self.tool,
            "severity": self.severity,
            "message": self.message,
        }
        # `file`/`line` omitidos quando ausentes — espelha o
        # `skip_serializing_if = Option::is_none` do Rust.
        if self.file is not None:
            d["file"] = self.file
        if self.line is not None:
            d["line"] = self.line
        return d


class VerificationStep(BaseModel):
    """Um passo do pipeline (typecheck/test/lint/sast)."""

    name: str
    tool: str
    exit_code: int
    duration_ms: int
    findings: list[Finding] = []

    @classmethod
    def from_proto(cls, msg: Any) -> "VerificationStep":
        return cls(
            name=msg.name,
            tool=msg.tool,
            exit_code=msg.exit_code,
            duration_ms=msg.duration_ms,
            findings=[Finding.from_proto(f) for f in msg.findings],
        )

    def to_wire_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "tool": self.tool,
            "exit_code": self.exit_code,
            "duration_ms": self.duration_ms,
            # `findings` sempre presente (Rust: `#[serde(default)]`, não skip).
            "findings": [f.to_wire_dict() for f in self.findings],
        }


class VerificationEvidence(BaseModel):
    """Evidência de verificação determinística — imutável."""

    model_config = {"frozen": True}

    run_id: str
    git_sha: str
    steps: list[VerificationStep] = []
    verdict: str  # "pass" | "fail" | "skipped"
    produced_at: str

    @classmethod
    def from_proto(cls, msg: Any) -> "VerificationEvidence":
        """Constrói da mensagem proto tipada. ``verdict`` UNSPECIFIED (0) =
        ``ValueError`` (fail-closed — evidência sem veredito válido não é
        evidência). A PRESENÇA do campo é checada no caller via ``HasField``:
        ausente = evidência faltando, tratada fail-closed lá."""
        verdict = _VERDICT_FROM_PROTO.get(msg.verdict)
        if verdict is None:
            raise ValueError(f"verdict inválido/ausente no wire: {msg.verdict!r}")
        return cls(
            run_id=msg.run_id,
            git_sha=msg.git_sha,
            steps=[VerificationStep.from_proto(s) for s in msg.steps],
            verdict=verdict,
            produced_at=msg.produced_at,
        )

    def to_wire_dict(self) -> dict[str, Any]:
        """Forma canônica do schema ``verification-evidence.v1`` — a mesma que
        o Rust serializa e que o caminho pré-D3t carregava como string."""
        return {
            "run_id": self.run_id,
            "git_sha": self.git_sha,
            "steps": [s.to_wire_dict() for s in self.steps],
            "verdict": self.verdict,
            "produced_at": self.produced_at,
        }
