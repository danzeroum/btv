import asyncio

import pytest

from btv_squad.evaluation import ContinuousEvaluator


def test_technical_score_deriva_da_confianca_real_quando_sucesso():
    evaluator = ContinuousEvaluator()
    result = asyncio.run(evaluator.evaluate_agent_performance("architect", {"success": True, "confidence": 0.85}))
    # Não o default fabricado 0.8 da origem — deriva da confiança real.
    assert result["technical_score"] == 0.85


def test_technical_score_e_zero_quando_falha():
    evaluator = ContinuousEvaluator()
    result = asyncio.run(evaluator.evaluate_agent_performance("developer", {"success": False, "confidence": 0.9}))
    assert result["technical_score"] == 0.0


def test_improvement_e_zero_na_primeira_avaliacao():
    evaluator = ContinuousEvaluator()
    result = asyncio.run(evaluator.evaluate_agent_performance("architect", {"success": True, "confidence": 0.7}))
    assert result["improvement"] == 0.0


def test_improvement_e_delta_contra_media_historica_real():
    evaluator = ContinuousEvaluator()
    asyncio.run(evaluator.evaluate_agent_performance("architect", {"success": True, "confidence": 0.5}))
    # baseline agora = média de [0.5] = 0.5; próxima avaliação com 0.8 → +0.3
    second = asyncio.run(evaluator.evaluate_agent_performance("architect", {"success": True, "confidence": 0.8}))
    assert second["improvement"] == pytest.approx(0.3)


def test_metricas_rolantes_registram_valores_reais():
    evaluator = ContinuousEvaluator()
    asyncio.run(evaluator.evaluate_agent_performance("architect", {"success": True, "confidence": 0.9}))
    asyncio.run(evaluator.evaluate_agent_performance("developer", {"success": False, "confidence": 0.3}))
    assert evaluator.metrics["task_success_rate"] == [1.0, 0.0]
    assert evaluator.metrics["average_confidence"] == [0.9, 0.3]
