"""Paridade e fail-closed da evidência de verificação TIPADA (D3t).

A fixture `schemas/fixtures/verification-evidence.v1.example.json` é o juiz
compartilhado com o lado Rust (`squad.rs::evidence_to_proto_espelha_a_fixture_v1`):
o mesmo conteúdo que o caminho pré-D3t carregava como string JSON, agora
tipado no wire. Prova que `VerificationEvidence.from_proto(...).to_wire_dict()`
reproduz o JSON canônico do schema — o prompt do auditor não muda.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from btv_proto import squad_pb2
from btv_squad.verification import VerificationEvidence

_FIXTURE = (
    Path(__file__).resolve().parents[4]
    / "schemas"
    / "fixtures"
    / "verification-evidence.v1.example.json"
)

_VERDICT_TO_PROTO = {
    "pass": squad_pb2.Verdict.VERDICT_PASS,
    "fail": squad_pb2.Verdict.VERDICT_FAIL,
    "skipped": squad_pb2.Verdict.VERDICT_SKIPPED,
}


def _fixture() -> dict:
    return json.loads(_FIXTURE.read_text(encoding="utf-8"))


def _proto_from_fixture(fx: dict) -> squad_pb2.VerificationEvidence:
    ev = squad_pb2.VerificationEvidence(
        run_id=fx["run_id"],
        git_sha=fx["git_sha"],
        verdict=_VERDICT_TO_PROTO[fx["verdict"]],
        produced_at=fx["produced_at"],
    )
    for s in fx["steps"]:
        step = ev.steps.add()
        step.name = s["name"]
        step.tool = s["tool"]
        step.exit_code = s["exit_code"]
        step.duration_ms = s["duration_ms"]
        for f in s.get("findings", []):
            fd = step.findings.add()
            fd.tool = f["tool"]
            fd.severity = f["severity"]
            fd.message = f["message"]
            # `optional` no proto — só seta quando presente na fixture, para
            # `HasField` refletir a ausência (paridade com o `Option` do Rust).
            if "file" in f:
                fd.file = f["file"]
            if "line" in f:
                fd.line = f["line"]
    return ev


def test_to_wire_dict_reproduz_a_fixture_canonica():
    fx = _fixture()
    evidence = VerificationEvidence.from_proto(_proto_from_fixture(fx))
    # Igualdade do JSON completo — inclusive `file`/`line` omitidos no achado
    # sem localização (paridade com o `skip_serializing_if` do Rust).
    assert evidence.to_wire_dict() == fx


def test_verdict_unspecified_e_recusado_fail_closed():
    # Mensagem presente mas com verdict zero-value (UNSPECIFIED) = evidência
    # sem veredito válido; `from_proto` recusa em vez de fabricar.
    ev = squad_pb2.VerificationEvidence(run_id="x", git_sha="y", produced_at="z")
    with pytest.raises(ValueError):
        VerificationEvidence.from_proto(ev)


def test_request_sem_evidencia_e_ausente():
    from btv_squad.server import _verification_evidence_from_request

    req = squad_pb2.SquadTask(task_id="t")  # verification_evidence não setado
    evidence, missing = _verification_evidence_from_request(req)
    assert evidence is None
    assert missing is True


def test_request_com_evidencia_valida_desce_canonica():
    from btv_squad.server import _verification_evidence_from_request

    fx = _fixture()
    req = squad_pb2.SquadTask(task_id="t")
    req.verification_evidence.CopyFrom(_proto_from_fixture(fx))
    evidence, missing = _verification_evidence_from_request(req)
    assert missing is False
    assert evidence == fx
