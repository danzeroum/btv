"""Geradores declarativos de prompt (origem: prompte `generators.js`).

Cada gerador declara `{name, fields, build(data)}` — módulo puro e
testável. O catálogo completo (25 templates em 4 categorias) migra na
Fase 3; aqui ficam os primeiros exemplares que fixam o formato
(`prompt-template.v1`).
"""

from __future__ import annotations

from collections.abc import Callable

from pydantic import BaseModel, Field as PydanticField


class Field(BaseModel):
    """Campo de entrada de um gerador."""

    name: str
    label: str
    required: bool = True
    placeholder: str = ""


class Generator(BaseModel):
    """Template declarativo: campos + função de montagem."""

    name: str
    category: str
    fields: list[Field]
    build: Callable[[dict[str, str]], str]

    def render(self, data: dict[str, str]) -> str:
        missing = [f.name for f in self.fields if f.required and not data.get(f.name)]
        if missing:
            raise ValueError(f"campos obrigatórios ausentes: {', '.join(missing)}")
        return self.build(data)


def _build_code_review(data: dict[str, str]) -> str:
    return (
        f"Revise o seguinte código {data['language']} com foco em correção, "
        f"legibilidade e segurança. Aponte problemas concretos com linha e "
        f"sugestão de correção.\n\nContexto: {data['context']}\n\n"
        f"```{data['language']}\n{data['code']}\n```"
    )


def _build_bug_fix(data: dict[str, str]) -> str:
    return (
        f"O código abaixo apresenta o seguinte comportamento incorreto: "
        f"{data['symptom']}\n\nComportamento esperado: {data['expected']}\n\n"
        f"Diagnostique a causa raiz antes de propor a correção mínima.\n\n"
        f"```\n{data['code']}\n```"
    )


GENERATORS: dict[str, Generator] = {
    g.name: g
    for g in [
        Generator(
            name="code-review",
            category="codigo",
            fields=[
                Field(name="language", label="Linguagem"),
                Field(name="context", label="Contexto do projeto"),
                Field(name="code", label="Código a revisar"),
            ],
            build=_build_code_review,
        ),
        Generator(
            name="bug-fix",
            category="codigo",
            fields=[
                Field(name="symptom", label="Sintoma observado"),
                Field(name="expected", label="Comportamento esperado"),
                Field(name="code", label="Código relevante"),
            ],
            build=_build_bug_fix,
        ),
    ]
}
