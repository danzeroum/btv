"""Testes do extrator de JSON compartilhado (`btv_squad._json`)."""

from __future__ import annotations

from btv_squad._json import extract_json_object


def test_extrai_objeto_simples():
    assert extract_json_object('{"a": 1, "b": "x"}') == {"a": 1, "b": "x"}


def test_ignora_prosa_com_chave_no_fim():
    # Regressão da regex gulosa `\{.*\}` + DOTALL: ela capturava até a ÚLTIMA
    # `}` da resposta e corrompia o parse quando havia prosa com chave depois.
    texto = 'Aqui está: {"acao": "ok"} — obrigado :}'
    assert extract_json_object(texto) == {"acao": "ok"}


def test_ignora_prefixo_antes_do_objeto():
    assert extract_json_object('blá blá {"n": 2} fim') == {"n": 2}


def test_sem_bloco_retorna_vazio():
    assert extract_json_object("nenhum json aqui") == {}


def test_json_invalido_retorna_vazio():
    assert extract_json_object('{"a": }') == {}


def test_topo_nao_objeto_retorna_vazio():
    assert extract_json_object("[1, 2, 3]") == {}
