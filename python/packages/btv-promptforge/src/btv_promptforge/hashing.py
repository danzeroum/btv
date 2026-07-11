"""Hash de cache de prompt (`prompt-cache-key.v1`).

Contrato herdado do prompte (`api/src/hash.js`): JSON canônico com chaves
ordenadas em todos os níveis, sem espaços, sha256 em hex minúsculo. A
implementação Rust equivalente vive em `btv-schemas::canonical`; a
paridade é garantida pelas fixtures em `platform/schemas/fixtures/`.

Restrição v1 (ADR 0032): floats com parte fracionária zero (ex.: 1.0) são
proibidos nas entradas — JS os serializa como "1", Rust/Python como "1.0".
Antes a regra era só prosa; agora `request_hash` a ENFORÇA (levanta
`CacheKeyError`), espelhando o guard Rust em `btv_schemas::canonical`.
"""

from __future__ import annotations

import hashlib
import json
import math
from typing import Any


class CacheKeyError(ValueError):
    """Entrada recusada pelo contrato `prompt-cache-key.v1`: contém um número
    que divergiria entre produtores (JS × Rust/Python). Ver ADR 0032."""


def canonical_json(value: Any) -> str:
    """JSON canônico: chaves ordenadas, separadores compactos, UTF-8 cru."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_hex(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _reject_forbidden_numbers(value: Any, path: str = "$") -> None:
    """Rejeita números que não sobrevivem à fronteira de produtores no v1:
    floats com fração zero (1.0 → JS "1", Python/Rust "1.0") e não-finitos
    (NaN/Inf). `bool` é subclasse de `int`, mas não de `float` — passa."""
    if isinstance(value, bool):
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise CacheKeyError(f"número não-finito proibido em {path}: {value!r}")
        if value.is_integer():
            raise CacheKeyError(
                f"float com fração zero proibido em {path}: {value!r}; "
                f"use o inteiro {int(value)} (JS serializa 1.0 como '1')"
            )
        return
    if isinstance(value, dict):
        for key, item in value.items():
            _reject_forbidden_numbers(item, f"{path}.{key}")
    elif isinstance(value, (list, tuple)):
        for i, item in enumerate(value):
            _reject_forbidden_numbers(item, f"{path}[{i}]")


def validate_cache_key(messages: Any, temperature: Any) -> None:
    """Valida as entradas do `prompt-cache-key.v1` (o mesmo guard que
    `request_hash` aplica). Levanta `CacheKeyError` se proibido."""
    _reject_forbidden_numbers(messages, "$.messages")
    _reject_forbidden_numbers(temperature, "$.temperature")


def request_hash(messages: Any, temperature: Any) -> str:
    """Hash do request de LLM — idêntico ao `btv_schemas::request_hash` (Rust).

    Levanta `CacheKeyError` para entradas proibidas pelo v1 (ex.: 1.0)."""
    validate_cache_key(messages, temperature)
    return sha256_hex(canonical_json({"messages": messages, "temperature": temperature}))
