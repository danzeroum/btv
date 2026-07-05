"""Squad multi-agente da plataforma Forge (sidecar Python).

Migração do protótipo BuildToValue (`src/` do repositório): consenso
ponderado, planejamento, roteamento, memória, HITL e fallback progressivo.
Regra de ouro: este pacote NUNCA chama provedores LLM diretamente — toda
geração passa pelo gateway Rust via gRPC (`CoreService.Generate`).
"""

from forge_squad.consensus import ConsensusResult, WeightedConsensusEngine

__all__ = ["ConsensusResult", "WeightedConsensusEngine"]
