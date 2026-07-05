"""Agentes especializados do squad (migrado de BuildToValue `src/agents/`).

Cada agente herda de `BaseAgent` e chama o LLM via `forge_squad.gateway.GatewayClient`
— nunca heurística local (princípio "Nada Fake", ADR 0005).
"""

from forge_squad.agents.architect import ArchitectAgent
from forge_squad.agents.base import BaseAgent

__all__ = ["ArchitectAgent", "BaseAgent"]
