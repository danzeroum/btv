"""Extração robusta de um único objeto JSON de respostas de modelo.

Centraliza o parse que estava duplicado nos agentes (``architect``, ``auditor``,
``developer``, ``ops``, ``designer``) e no ``planning``. Além de DRY, corrige a
regex gulosa ``\\{.*\\}`` (com ``re.DOTALL`` ela capturava do primeiro ``{`` até
a ÚLTIMA ``}`` da resposta inteira — qualquer chave em prosa posterior corrompia
o parse): aqui varremos a partir do primeiro ``{`` e deixamos o ``raw_decode``
do decoder achar o fim REAL do objeto.
"""

from __future__ import annotations

import json
import logging
from typing import Any

logger = logging.getLogger(__name__)

_DECODER = json.JSONDecoder()


def extract_json_object(raw_text: str, *, context: str = "") -> dict[str, Any]:
    """Extrai o primeiro objeto JSON de ``raw_text``.

    Retorna ``{}`` (com log de aviso) quando não há objeto, quando o JSON é
    inválido ou quando o valor de topo não é um objeto — o mesmo contrato
    defensivo que cada agente já usava: uma resposta malformada nunca derruba
    o agente.
    """

    rotulo = f" ({context})" if context else ""
    inicio = raw_text.find("{")
    if inicio == -1:
        logger.warning("Resposta do modelo%s não contém um bloco JSON: %r", rotulo, raw_text[:200])
        return {}
    try:
        candidate, _ = _DECODER.raw_decode(raw_text[inicio:])
    except json.JSONDecodeError:
        logger.warning("Resposta do modelo%s não é JSON válido: %r", rotulo, raw_text[:200])
        return {}
    return candidate if isinstance(candidate, dict) else {}
