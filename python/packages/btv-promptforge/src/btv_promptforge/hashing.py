"""Hash de cache de prompt (`prompt-cache-key.v1`).

Contrato herdado do prompte (`api/src/hash.js`): JSON canônico com chaves
ordenadas em todos os níveis, sem espaços, sha256 em hex minúsculo. A
implementação Rust equivalente vive em `btv-schemas::canonical`; a
paridade é garantida pelas fixtures em `platform/schemas/fixtures/`.

Restrição v1: floats com parte fracionária zero (ex.: 1.0) são proibidos
nas entradas — JS os serializa como "1", Rust/Python como "1.0".
"""

from __future__ import annotations

import hashlib
import json
from typing import Any


def canonical_json(value: Any) -> str:
    """JSON canônico: chaves ordenadas, separadores compactos, UTF-8 cru."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_hex(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def request_hash(messages: Any, temperature: Any) -> str:
    """Hash do request de LLM — idêntico ao `btv_schemas::request_hash` (Rust)."""
    return sha256_hex(canonical_json({"messages": messages, "temperature": temperature}))
