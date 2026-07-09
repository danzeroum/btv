"""Classe base compartilhada pelos agentes do squad (migrado de
BuildToValue `src/agents/base_agent.py`).
"""

from __future__ import annotations

import logging
import uuid
from abc import ABC, abstractmethod
from datetime import datetime, timezone
from typing import Any, Optional

from btv_squad.gateway import GatewayClient

logger = logging.getLogger(__name__)


class BaseAgent(ABC):
    """Comportamento comum aos agentes especializados do squad."""

    def __init__(self, agent_type: str) -> None:
        self.agent_type = agent_type
        self.agent_id = str(uuid.uuid4())
        self.created_at = datetime.now(timezone.utc)
        self.confidence_threshold = 0.7
        self.memory: Any = None  # injetado preguiçosamente pelo orquestrador
        #: Injetado preguiçosamente pelo orquestrador (ADR 0005) — nenhum
        #: agente fala com o gateway antes de attach_gateway() ser chamado.
        self.gateway: Optional[GatewayClient] = None
        self.tools: list[str] = []
        #: System prompt da PERSONA (U7) em vigor nesta ativação — o roster do
        #: `SquadTask` o injeta (Fase 1). Quando presente, é PREPENDIDO ao
        #: system prompt operacional do agente (voz/objetivo da persona +
        #: protocolo JSON/ferramentas do agente), em vez de substituí-lo — assim
        #: editar a persona no frontend muda de fato como o agente trabalha, sem
        #: quebrar o contrato de saída. `None` = comportamento padrão do motor.
        self.persona_prompt: Optional[str] = None

    def system_with_persona(self, base: str) -> str:
        """Combina o prompt da persona (se houver) com o system prompt
        operacional do agente. A persona vem primeiro (voz/objetivo); o `base`
        (JSON/ferramentas) permanece para o contrato de saída não quebrar."""

        persona = (self.persona_prompt or "").strip()
        return f"{persona}\n\n{base}" if persona else base

    @abstractmethod
    async def execute(self, task: dict[str, Any]) -> dict[str, Any]:
        """Executa a responsabilidade principal do agente."""

    def validate_input(self, task: dict[str, Any]) -> bool:
        """Checagem básica de que os metadados exigidos existem."""

        return "description" in task and bool(task["description"])

    def log_decision(self, decision: dict[str, Any]) -> dict[str, Any]:
        """Persiste a decisão na memória e registra no log."""

        entry = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "agent": self.agent_type,
            "agent_id": self.agent_id,
            "decision": decision,
        }

        if self.memory:
            try:
                self.memory.remember_decision(self.agent_type, entry)
            except Exception as exc:  # pragma: no cover - defensivo
                logger.warning("Não foi possível persistir a memória do agente", exc_info=exc)

        logger.info("%s registrou uma decisão", self.agent_type, extra={"decision": entry})
        return entry

    def attach_memory(self, memory: Any) -> None:
        """Permite ao orquestrador injetar o backend de memória."""

        self.memory = memory

    def attach_gateway(self, gateway: GatewayClient) -> None:
        """Injeta o cliente do gateway LLM (ADR 0005)."""

        self.gateway = gateway

    def validate_confidence(self, confidence: Optional[float]) -> bool:
        """Helper de conveniência para subclasses avaliarem respostas."""

        if confidence is None:
            return False
        return confidence >= self.confidence_threshold
