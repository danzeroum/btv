import json
from pathlib import Path

import pytest

from btv_promptforge.hashing import (
    CacheKeyError,
    canonical_json,
    request_hash,
    sha256_hex,
    validate_cache_key,
)

FIXTURES = Path(__file__).resolve().parents[4] / "schemas" / "fixtures" / "prompt-cache-key.v1.json"


def test_canonico_ordena_chaves_em_todos_os_niveis():
    value = {"b": {"z": 1, "a": [True, None]}, "a": "x"}
    assert canonical_json(value) == '{"a":"x","b":{"a":[true,null],"z":1}}'


def test_sha256_conhecido():
    assert sha256_hex("abc") == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"


def test_paridade_com_fixtures_compartilhadas():
    """As mesmas fixtures são validadas pelo teste Rust em btv-schemas.

    Qualquer divergência aqui significa quebra do contrato
    `prompt-cache-key.v1` entre o gateway (Rust) e o sidecar (Python).
    """
    doc = json.loads(FIXTURES.read_text(encoding="utf-8"))
    cases = doc["cases"]
    assert len(cases) >= 5
    for case in cases:
        got = request_hash(case["messages"], case["temperature"])
        assert got == case["sha256"], f"fixture {case['name']}: {got} != {case['sha256']}"

    # Casos PROIBIDOS (ADR 0032): os MESMOS reject_cases que o Rust recusa
    # (parity.rs) devem levantar CacheKeyError aqui.
    reject = doc["reject_cases"]
    assert reject
    for case in reject:
        with pytest.raises(CacheKeyError):
            request_hash(case["messages"], case["temperature"])


def test_rejeita_float_com_fracao_zero():
    with pytest.raises(CacheKeyError):
        request_hash([{"role": "user", "content": "oi"}], 1.0)
    with pytest.raises(CacheKeyError):
        request_hash([{"role": "user", "content": "oi"}], 0.0)
    # Aninhado dentro de messages.
    with pytest.raises(CacheKeyError):
        request_hash([{"n": 3.0}], 0.5)


def test_rejeita_nao_finito():
    # Diferente do Rust (serde_json não carrega Inf/NaN), aqui float('inf') é um
    # float real e o guard o recusa.
    with pytest.raises(CacheKeyError):
        request_hash([], float("inf"))
    with pytest.raises(CacheKeyError):
        request_hash([], float("nan"))


def test_aceita_int_bool_e_float_nao_inteiro():
    # int 1, bool, None e float não inteiro passam sem levantar.
    validate_cache_key([{"n": 42, "flag": True, "x": None}], 1)
    validate_cache_key([{"role": "user", "content": "oi"}], 0.7)
