"""Execução paralela respeitando limites de recurso (migrado de
BuildToValue `src/parallel/resource_manager.py`).

Diferente dos agentes e do planner, isto é infraestrutura determinística
legítima (semáforo + `asyncio.gather`) — plumbing, não raciocínio. Não
tem chamada de gateway aqui, e não deveria ter: forçar uma decisão de
LLM sobre "quantas tarefas rodar em paralelo" seria fabricar uma decisão
onde já existe uma resposta mecânica correta. Mesmo espírito do
`sandbox`/`security` da Onda 1.
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Any, Awaitable, Callable, Iterable

TaskLike = Awaitable[Any] | Callable[[], Awaitable[Any]]


@dataclass
class ParallelResourceManager:
    """Executa tarefas assíncronas respeitando limites básicos de recurso."""

    limits: dict[str, float] = field(default_factory=lambda: {"max_concurrent": 5})

    async def execute_parallel_with_limits(self, tasks: Iterable[TaskLike]) -> list[Any]:
        semaphore = asyncio.Semaphore(int(self.limits.get("max_concurrent", 5)))

        async def _ensure(task: TaskLike) -> Any:
            if callable(task):
                return await task()
            return await task

        async def _run(task: TaskLike) -> Any:
            async with semaphore:
                return await _ensure(task)

        return await asyncio.gather(*[_run(task) for task in tasks])
