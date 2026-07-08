import pytest

from btv_promptforge.generators import GENERATORS


def test_code_review_monta_prompt_com_todos_os_campos():
    prompt = GENERATORS["code-review"].render(
        {"language": "rust", "context": "gateway LLM", "code": "fn main() {}"}
    )
    assert "rust" in prompt
    assert "gateway LLM" in prompt
    assert "fn main() {}" in prompt


def test_campo_obrigatorio_ausente_falha():
    with pytest.raises(ValueError, match="code"):
        GENERATORS["bug-fix"].render({"symptom": "panica", "expected": "não panicar"})
