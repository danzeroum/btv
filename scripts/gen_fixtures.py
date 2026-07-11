"""Regenera as fixtures de paridade do contrato prompt-cache-key.v1.

As fixtures são a fonte de verdade do teste de contrato cross-language:
- Rust: crates/btv-schemas/tests/parity.rs
- Python: python/packages/btv-promptforge/tests/test_hashing.py

Rode via `just gen-fixtures` (usa a implementação de referência do
btv_promptforge, não uma cópia local do algoritmo).
"""

from __future__ import annotations

import json
from pathlib import Path

from btv_promptforge.hashing import CacheKeyError, request_hash

CASES = [
    {"name": "simples", "messages": [{"role": "user", "content": "oi"}], "temperature": 0.7},
    {"name": "temperatura-nula", "messages": [{"role": "user", "content": "oi"}], "temperature": None},
    {
        "name": "chaves-fora-de-ordem",
        "messages": [{"role": "system", "content": "seja conciso", "z": 1, "a": 2}],
        "temperature": 0.3,
    },
    {
        "name": "acentuacao-utf8",
        "messages": [{"role": "user", "content": "canonicalização não-ASCII: ção, ñ, 中文"}],
        "temperature": 0.5,
    },
    {
        "name": "multi-turno-aninhado",
        "messages": [
            {"role": "user", "content": "a"},
            {"role": "assistant", "content": "b", "meta": {"tags": ["x", "y"], "n": 3}},
        ],
        "temperature": 0.2,
    },
    {
        "name": "escapes-json",
        "messages": [{"role": "user", "content": 'linha1\nlinha2\t"aspas" e \\barra'}],
        "temperature": 0.9,
    },
    {
        "name": "inteiros-e-booleans",
        "messages": [{"role": "user", "content": "x", "count": 42, "flag": True, "none": None}],
        "temperature": 1,
    },
]

# Entradas PROIBIDAS pelo v1 (ADR 0032): floats com fração zero divergem entre
# produtores (JS "1"/"0", Rust/Python "1.0"/"0.0"). Só valores representáveis em
# JSON cross-language entram aqui — NaN/Inf não são JSON válido (serde_json nem
# os carrega), então ficam em testes inline do lado Python.
REJECT_CASES = [
    {
        "name": "temperatura-float-inteira",
        "messages": [{"role": "user", "content": "oi"}],
        "temperature": 1.0,
    },
    {
        "name": "temperatura-zero-float",
        "messages": [{"role": "user", "content": "oi"}],
        "temperature": 0.0,
    },
    {
        "name": "float-inteiro-aninhado-em-messages",
        "messages": [{"role": "user", "content": "x", "n": 3.0}],
        "temperature": 0.5,
    },
]


def main() -> None:
    for case in CASES:
        case["sha256"] = request_hash(case["messages"], case["temperature"])
    # Auto-verificação: a impl de referência DEVE rejeitar cada reject_case —
    # senão a fixture mentiria e o teste de paridade passaria à toa.
    for rc in REJECT_CASES:
        try:
            request_hash(rc["messages"], rc["temperature"])
        except CacheKeyError:
            pass
        else:
            raise SystemExit(f"reject_case '{rc['name']}' NÃO foi rejeitado — fixture inválida")
    out = {
        "$comment": (
            "Fixtures de paridade do contrato prompt-cache-key.v1. Validadas por "
            "btv-schemas (Rust, tests/parity.rs) e btv_promptforge (Python, "
            "tests/test_hashing.py). `cases` = entradas válidas com sha256; "
            "`reject_cases` = entradas proibidas (ADR 0032) que ambos os lados "
            "devem recusar. Regeneração: just gen-fixtures."
        ),
        "cases": CASES,
        "reject_cases": REJECT_CASES,
    }
    path = Path(__file__).resolve().parents[1] / "schemas" / "fixtures" / "prompt-cache-key.v1.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"{len(CASES)} casos + {len(REJECT_CASES)} reject_cases escritos em {path}")


if __name__ == "__main__":
    main()
