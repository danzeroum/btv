"""Agentes especializados do squad (migrado de BuildToValue `src/agents/`).

Cada agente herda de `BaseAgent` e chama o LLM via `btv_squad.gateway.GatewayClient`
— nunca heurística local (princípio "Nada Fake", ADR 0005). O `UnifiedOrchestrator`
(Onda 4) instancia exatamente estes 5 — Supervisor/Exploration/Recovery não têm
chamador real na lineage canônica (ADR 0004).
"""

from btv_squad.agents.architect import ArchitectAgent
from btv_squad.agents.auditor import AuditorAgent
from btv_squad.agents.base import BaseAgent
from btv_squad.agents.designer import DesignerAgent
from btv_squad.agents.developer import DeveloperAgent, ReviewSystem
from btv_squad.agents.ops import OpsAgent

__all__ = [
    "ArchitectAgent",
    "AuditorAgent",
    "BaseAgent",
    "DesignerAgent",
    "DeveloperAgent",
    "OpsAgent",
    "ReviewSystem",
]
