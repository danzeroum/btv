"""Memória persistente de agentes (migrado de BuildToValue
`src/memory/agent_memory.py`). Curto/longo prazo + episódica em disco.

O **corpus episódico em disco** (o JSONL) é a fonte da verdade. A recuperação
(`recall_similar`) é feita por um índice TF-IDF local (`recall.py`, Fase 6
Onda 6). O scaffolding chromadb (`_FallbackCollection` no-op + `collection.add`
em `remember_decision`) foi removido na validação de pendencias.md: era um sink
inativo que nunca foi consultado — um vector DB real, se vier, é uma onda/ADR
nova (ADR 0013 registra o limite léxico do retriever atual). Diretório de
armazenamento segue a convenção `.btv/` do resto da plataforma (era
`.buildtoflip/ledger` na origem).
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

from . import recall


class AgentMemorySystem:
    """Gerencia memórias de curto, longo prazo e episódicas dos agentes."""

    def __init__(self, storage_dir: Optional[Path] = None) -> None:
        self.short_term: dict[str, Any] = {}
        self.storage_dir = storage_dir or Path(".btv") / "squad-memory"
        self.storage_dir.mkdir(parents=True, exist_ok=True)
        self.episodic_path = self.storage_dir / "agent_memories.jsonl"
        # Cache do corpus parseado — `_load_corpus` era relido e re-parseado do
        # disco a cada `recall_similar`/`list_memories` (O(N) por chamada). O
        # carimbo `(mtime_ns, tamanho)` invalida em append de QUALQUER processo,
        # não só do `remember_decision` local.
        self._corpus_cache: list[dict[str, Any]] = []
        self._corpus_stamp: Optional[tuple[int, int]] = None

    def _corpus_fingerprint(self) -> Optional[tuple[int, int]]:
        try:
            st = self.episodic_path.stat()
        except FileNotFoundError:
            return None
        return (st.st_mtime_ns, st.st_size)

    def remember_decision(self, agent: str, decision: dict[str, Any]) -> None:
        """Grava uma decisão importante no corpus episódico em disco."""

        memory = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "agent": agent,
            "decision": decision,
            "confidence": float(decision.get("confidence", 0.0)),
        }
        with self.episodic_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(memory, ensure_ascii=False) + "\n")

    def _load_corpus(self) -> list[dict[str, Any]]:
        """Lê o corpus episódico do disco (JSONL). É a fonte da verdade do
        recall — persiste entre sessões e já contém o que foi lembrado nesta
        (o `remember_decision` grava na hora). Linhas malformadas são puladas.

        O resultado é cacheado até o arquivo mudar (ver `_corpus_fingerprint`):
        chamadas repetidas sem append devolvem a mesma lista sem reler/re-parsear."""
        stamp = self._corpus_fingerprint()
        if stamp is None:
            self._corpus_cache = []
            self._corpus_stamp = None
            return self._corpus_cache
        if stamp == self._corpus_stamp:
            return self._corpus_cache
        records: list[dict[str, Any]] = []
        with self.episodic_path.open("r", encoding="utf-8") as handle:
            for line in handle:
                line = line.strip()
                if not line:
                    continue
                try:
                    rec = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if isinstance(rec, dict) and "decision" in rec:
                    records.append(rec)
        self._corpus_cache = records
        self._corpus_stamp = stamp
        return self._corpus_cache

    def list_memories(self, agent: Optional[str] = None, limit: int = 50) -> list[dict[str, Any]]:
        """Lista memórias persistidas, mais recentes primeiro, opcionalmente
        filtradas por agente (Fase 7 Onda 8, A3 — mapa de memória). Reusa
        `_load_corpus()` (fonte da verdade) — zero lógica de indexação nova,
        só filtro + ordenação + corte."""
        corpus = self._load_corpus()
        if agent:
            corpus = [rec for rec in corpus if rec.get("agent") == agent]
        return list(reversed(corpus))[:limit]

    def recall_similar(
        self, query: str, k: int = 5, embedder: "recall.Embedder | None" = None
    ) -> dict[str, Any]:
        """Recupera as `k` memórias mais similares à `query` sobre o corpus
        episódico (Fase 6 Onda 6 — recuperação real). Sem `embedder`, usa o
        índice LÉXICO TF-IDF-cosseno (default offline, zero-dep). Com um
        `embedder` neural injetado, usa recuperação SEMÂNTICA (cosseno de
        embeddings — casa sinônimo/paráfrase). Devolve listas paralelas
        (`ids`/`documents`/`metadatas`/`scores`) das relevantes, em ordem
        decrescente; vazio se nada casa."""
        corpus = self._load_corpus()
        docs = [json.dumps(rec.get("decision", {}), ensure_ascii=False) for rec in corpus]
        ranked = (
            recall.semantic_rank(query, docs, embedder, k)
            if embedder is not None
            else recall.rank(query, docs, k)
        )
        return {
            "ids": [
                f"{corpus[i].get('agent', '?')}_{corpus[i].get('timestamp', i)}"
                for i, _ in ranked
            ],
            "documents": [docs[i] for i, _ in ranked],
            "metadatas": [
                {"agent": corpus[i].get("agent"), "timestamp": corpus[i].get("timestamp")}
                for i, _ in ranked
            ],
            "scores": [score for _, score in ranked],
            "query": [query],
            "n_results": k,
        }
