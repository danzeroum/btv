from forge_promptforge.lint import lint_prompt


def test_prompt_vago_e_penalizado():
    report = lint_prompt("faça o melhor código rápido")
    rules = {issue.rule for issue in report.issues}
    assert "vague-term" in rules
    assert "missing-context" in rules
    assert report.score < 0.7


def test_prompt_concreto_passa():
    prompt = (
        "Revise a função abaixo do módulo de pagamento buscando erros de "
        "arredondamento em centavos. Entrada:\n```python\ndef soma(a, b): return a + b\n```"
    )
    report = lint_prompt(prompt)
    assert report.score >= 0.9
    assert report.grade == "A"
