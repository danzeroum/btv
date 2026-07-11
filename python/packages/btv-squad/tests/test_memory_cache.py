"""Cache do corpus episódico em `AgentMemorySystem._load_corpus`."""

from __future__ import annotations

from btv_squad.memory import AgentMemorySystem


def test_load_corpus_cacheia_ate_o_arquivo_mudar(tmp_path):
    mem = AgentMemorySystem(storage_dir=tmp_path)
    mem.remember_decision("architect", {"confidence": 0.9, "summary": "a"})

    c1 = mem._load_corpus()
    c2 = mem._load_corpus()
    # Cache hit: mesma lista, sem reler/re-parsear o disco.
    assert c1 is c2
    assert len(c1) == 1

    # Append muda (mtime_ns, tamanho) → o cache invalida na próxima leitura.
    mem.remember_decision("architect", {"confidence": 0.8, "summary": "b"})
    c3 = mem._load_corpus()
    assert c3 is not c1
    assert len(c3) == 2


def test_corpus_inexistente_e_vazio(tmp_path):
    mem = AgentMemorySystem(storage_dir=tmp_path)
    assert mem._load_corpus() == []
    # Continua consistente numa segunda chamada (stamp None).
    assert mem._load_corpus() == []
