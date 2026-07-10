"""Espelho Pydantic mínimo do ``TenantContext`` do domínio Rust (D4t).

O D2t fez ``tenant_id``/``actor`` fluírem pelo proto como duas strings
soltas; este módulo dá a elas UM tipo do lado Python — validação na
entrada (parse, don't validate) em vez de strings repassadas às cegas —
preparando o terreno da E1s sem adiantá-la: nenhum Protocol existente
muda, o orquestrador continua PROPAGANDO (nunca decidindo) o tenant.

Paridade com o Rust declarada (não fingida): ``TenantId`` do domínio
serializa como UUID canônico (minúsculo, hifenizado — ``uuid::Uuid``
Display) e é isso que os callers Rust EMITEM; a validação daqui exige
exatamente essa forma canônica. ``ActorId`` no Rust exige não-vazio;
idem aqui. O par vazio é o wire pré-D2t (sem contexto) e vira ``None`` —
nunca um contexto fabricado.
"""

from __future__ import annotations

import uuid
from typing import Optional

from pydantic import BaseModel, field_validator

#: O tenant do modo local — mesmo UUID fixo do `TenantId::LOCAL` (ADR 0025).
LOCAL_TENANT_ID = "00000000-0000-0000-0000-000000000001"


class TenantContext(BaseModel):
    """Dono (tenant) + operador (actor) de uma tarefa — imutável."""

    model_config = {"frozen": True}

    tenant_id: str
    actor: str

    @field_validator("tenant_id")
    @classmethod
    def _tenant_canonico(cls, v: str) -> str:
        try:
            parsed = uuid.UUID(v)
        except ValueError as exc:
            raise ValueError(f"tenant_id não é um UUID: {v!r}") from exc
        if str(parsed) != v:
            # A forma canônica é a que o Rust emite; aceitar variantes
            # (maiúsculas, chaves) quebraria o eco VERBATIM do D2t.
            raise ValueError(f"tenant_id fora da forma canônica: {v!r}")
        return v

    @field_validator("actor")
    @classmethod
    def _actor_nao_vazio(cls, v: str) -> str:
        if not v.strip():
            raise ValueError("actor vazio")
        return v

    @classmethod
    def from_wire(cls, tenant_id: str, actor: str) -> Optional["TenantContext"]:
        """Constrói do par do proto. Par VAZIO = wire pré-D2t (sem contexto)
        → ``None``, nunca um LOCAL fabricado (quem resolve tenant é a borda,
        ADR 0029). Parcialmente preenchido ou inválido = ``ValueError``
        (fail-closed — um caller que manda tenant sem actor está quebrado,
        não "quase certo")."""
        if not tenant_id and not actor:
            return None
        return cls(tenant_id=tenant_id, actor=actor)
