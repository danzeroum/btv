"""Camada de prompts da plataforma Forge (origem: prompte).

Geradores declarativos, base de conhecimento aditiva, quality linter
("ESLint para prompts") e o contrato de hash de cache compartilhado com o
gateway Rust (`prompt-cache-key.v1`).
"""

from forge_promptforge.generators import Field as GeneratorField
from forge_promptforge.generators import Generator, GENERATORS
from forge_promptforge.hashing import canonical_json, request_hash, sha256_hex
from forge_promptforge.lint import lint_prompt

__all__ = [
    "GENERATORS",
    "Generator",
    "GeneratorField",
    "canonical_json",
    "lint_prompt",
    "request_hash",
    "sha256_hex",
]
