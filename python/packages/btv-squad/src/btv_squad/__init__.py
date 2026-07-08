"""Squad multi-agente da plataforma BuildToValue (sidecar Python).

Migração do protótipo BuildToValue (`src/` do repositório): consenso
ponderado, planejamento, roteamento, memória, HITL e fallback progressivo.
Regra de ouro: este pacote NUNCA chama provedores LLM diretamente — toda
geração passa pelo gateway Rust via gRPC (`CoreService.Generate`).
"""

from btv_squad.agents import (
    ArchitectAgent,
    AuditorAgent,
    BaseAgent,
    DesignerAgent,
    DeveloperAgent,
    OpsAgent,
    ReviewSystem,
)
from btv_squad.chains import ChainStep, ResilientPromptChain
from btv_squad.consensus import ConsensusResult, WeightedConsensusEngine
from btv_squad.evaluation import ContinuousEvaluator
from btv_squad.forgetting import IntelligentForgetting, MemoryStore
from btv_squad.gateway import GatewayClient, LlmRequest, LlmResponse, ScriptedGatewayClient
from btv_squad.hitl import ProgressiveAutonomyManager
from btv_squad.memory import AgentMemorySystem
from btv_squad.orchestrator import UnifiedOrchestrator
from btv_squad.parallel import ParallelResourceManager
from btv_squad.permission import (
    PermissionClient,
    PermissionDecision,
    PermissionRequest,
    ScriptedPermissionClient,
)
from btv_squad.planning import AdaptivePlanner
from btv_squad.routing import LearningRouter
from btv_squad.sandbox import DockerSandbox, SecureToolSandbox, SecurityError
from btv_squad.security import SecurityConfig

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
