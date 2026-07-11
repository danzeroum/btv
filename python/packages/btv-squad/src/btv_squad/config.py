"""Configuração do sidecar de squad (12-Factor: valores overriden por env)."""

from __future__ import annotations

import os

#: Modelo LLM padrão dos agentes quando nenhum é passado explicitamente. Antes
#: era o literal ``"claude-sonnet-5"`` repetido em ~10 sítios; centralizado aqui
#: e overridável por ambiente sem recompilar: ``BTV_SQUAD_MODEL=...``.
DEFAULT_MODEL = os.getenv("BTV_SQUAD_MODEL", "claude-sonnet-5")
