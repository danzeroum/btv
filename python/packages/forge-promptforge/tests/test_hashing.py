import json
from pathlib import Path

from forge_promptforge.hashing import canonical_json, request_hash, sha256_hex

FIXTURES = Path(__file__).resolve().parents[4] / "schemas" / "fixtures" / "prompt-cache-key.v1.json"


def test_canonico_ordena_chaves_em_todos_os_niveis():
    value = {"b": {"z": 1, "a": [True, None]}, "a": "x"}
    assert canonical_json(value) == '{"a":"x","b":{"a":[true,null],"z":1}}'


def test_sha256_conhecido():
    assert sha256_hex("abc") == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"


def test_paridade_com_fixtures_compartilhadas():
    """As mesmas fixtures são validadas pelo teste Rust em forge-schemas.

    Qualquer divergência aqui significa quebra do contrato
    `prompt-cache-key.v1` entre o gateway (Rust) e o sidecar (Python).
    """
    cases = json.loads(FIXTURES.read_text(encoding="utf-8"))["cases"]
    assert len(cases) >= 5
    for case in cases:
        got = request_hash(case["messages"], case["temperature"])
        assert got == case["sha256"], f"fixture {case['name']}: {got} != {case['sha256']}"
