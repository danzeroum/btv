"""Squad multi-agente da plataforma Forge (sidecar Python).

Migração do protótipo BuildToValue (`src/` do repositório): consenso
ponderado, planejamento, roteamento, memória, HITL e fallback progressivo.
Regra de ouro: este pacote NUNCA chama provedores LLM diretamente — toda
geração passa pelo gateway Rust via gRPC (`CoreService.Generate`).
"""

from forge_squad.agents import (
    ArchitectAgent,
    AuditorAgent,
    BaseAgent,
    DesignerAgent,
    DeveloperAgent,
    OpsAgent,
    ReviewSystem,
)
from forge_squad.chains import ChainStep, ResilientPromptChain
from forge_squad.consensus import ConsensusResult, WeightedConsensusEngine
from forge_squad.evaluation import ContinuousEvaluator
from forge_squad.forgetting import IntelligentForgetting, MemoryStore
from forge_squad.gateway import GatewayClient, LlmRequest, LlmResponse, ScriptedGatewayClient
from forge_squad.hitl import ProgressiveAutonomyManager
from forge_squad.memory import AgentMemorySystem
from forge_squad.orchestrator import UnifiedOrchestrator
from forge_squad.parallel import ParallelResourceManager
from forge_squad.permission import (
    PermissionClient,
    PermissionDecision,
    PermissionRequest,
    ScriptedPermissionClient,
)
from forge_squad.planning import AdaptivePlanner
from forge_squad.routing import LearningRouter
from forge_squad.sandbox import DockerSandbox, SecureToolSandbox, SecurityError
from forge_squad.security import SecurityConfig

__all__ = [
    "AdaptivePlanner",
    "AgentMemorySystem",
    "ArchitectAgent",
    "AuditorAgent",
    "BaseAgent",
    "ChainStep",
    "ConsensusResult",
    "ContinuousEvaluator",
    "DesignerAgent",
    "DeveloperAgent",
    "DockerSandbox",
    "GatewayClient",
    "IntelligentForgetting",
    "LearningRouter",
    "LlmRequest",
    "LlmResponse",
    "MemoryStore",
    "OpsAgent",
    "ParallelResourceManager",
    "PermissionClient",
    "PermissionDecision",
    "PermissionRequest",
    "ProgressiveAutonomyManager",
    "ResilientPromptChain",
    "ReviewSystem",
    "ScriptedGatewayClient",
    "ScriptedPermissionClient",
    "SecureToolSandbox",
    "SecurityConfig",
    "SecurityError",
    "UnifiedOrchestrator",
    "WeightedConsensusEngine",
]
