"""Agentes especializados do squad (migrado de BuildToValue `src/agents/`).

Cada agente herda de `BaseAgent` e chama o LLM via `forge_squad.gateway.GatewayClient`
— nunca heurística local (princípio "Nada Fake", ADR 0005). O `UnifiedOrchestrator`
(Onda 4) instancia exatamente estes 5 — Supervisor/Exploration/Recovery não têm
chamador real na lineage canônica (ADR 0004).
"""

from forge_squad.agents.architect import ArchitectAgent
from forge_squad.agents.auditor import AuditorAgent
from forge_squad.agents.base import BaseAgent
from forge_squad.agents.designer import DesignerAgent
from forge_squad.agents.developer import DeveloperAgent, ReviewSystem
from forge_squad.agents.ops import OpsAgent

__all__ = [
    "ArchitectAgent",
    "AuditorAgent",
    "BaseAgent",
    "DesignerAgent",
    "DeveloperAgent",
    "OpsAgent",
    "ReviewSystem",
]
