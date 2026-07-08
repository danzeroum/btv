import asyncio

from btv_squad.parallel import ParallelResourceManager


def test_executa_callables_assincronos_e_preserva_ordem():
    async def make(n):
        return n * 2

    manager = ParallelResourceManager()
    results = asyncio.run(manager.execute_parallel_with_limits([lambda: make(1), lambda: make(2), lambda: make(3)]))

    assert results == [2, 4, 6]


def test_executa_corrotinas_diretamente():
    async def coro(n):
        return n + 1

    manager = ParallelResourceManager()
    results = asyncio.run(manager.execute_parallel_with_limits([coro(1), coro(2)]))

    assert results == [2, 3]


def test_respeita_o_limite_de_concorrencia():
    concurrent = 0
    max_seen = 0
    lock = asyncio.Lock()

    async def task():
        nonlocal concurrent, max_seen
        async with lock:
            concurrent += 1
            max_seen = max(max_seen, concurrent)
        await asyncio.sleep(0.01)
        async with lock:
            concurrent -= 1

    async def run():
        manager = ParallelResourceManager(limits={"max_concurrent": 2})
        await manager.execute_parallel_with_limits([task for _ in range(6)])

    asyncio.run(run())
    assert max_seen <= 2
