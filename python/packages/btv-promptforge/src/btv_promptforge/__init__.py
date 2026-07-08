"""Camada de prompts da plataforma BuildToValue (origem: prompte).

Geradores declarativos, base de conhecimento aditiva, quality linter
("ESLint para prompts") e o contrato de hash de cache compartilhado com o
gateway Rust (`prompt-cache-key.v1`).
"""

from btv_promptforge.generators import Field as GeneratorField
from btv_promptforge.generators import Generator, GENERATORS
from btv_promptforge.hashing import canonical_json, request_hash, sha256_hex
from btv_promptforge.lint import lint_prompt

__all__ = [
    "GENERATORS",
    "Generator",
    "GeneratorField",
    "canonical_json",
    "lint_prompt",
    "request_hash",
    "sha256_hex",
]
