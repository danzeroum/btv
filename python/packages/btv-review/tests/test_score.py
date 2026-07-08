from btv_review.score import ReviewScores, is_approved, value_score


def test_pesos_somam_um_e_score_maximo_e_um():
    scores = ReviewScores(technical=1.0, performance=1.0, security=1.0, value=1.0)
    assert value_score(scores) == 1.0


def test_seguranca_pesa_mais():
    so_seguranca = ReviewScores(technical=0.0, performance=0.0, security=1.0, value=0.0)
    so_performance = ReviewScores(technical=0.0, performance=1.0, security=0.0, value=0.0)
    assert value_score(so_seguranca) > value_score(so_performance)


def test_aprovacao_exige_score_acima_de_070():
    reprovado = ReviewScores(technical=0.7, performance=0.7, security=0.7, value=0.7)
    assert not is_approved(reprovado)  # exatamente 0.7 não passa
    aprovado = ReviewScores(technical=0.9, performance=0.8, security=0.9, value=0.8)
    assert is_approved(aprovado)
