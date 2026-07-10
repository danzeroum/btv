"""D4t mínimo: o espelho Pydantic do TenantContext — validação canônica
(paridade declarada com o que o Rust EMITE), par vazio = pré-D2t, inválido
= fail-closed."""

import pytest

from btv_squad.tenant import LOCAL_TENANT_ID, TenantContext


def test_local_canonico_constroi():
    ctx = TenantContext.from_wire(LOCAL_TENANT_ID, "cli:squad")
    assert ctx is not None
    assert ctx.tenant_id == LOCAL_TENANT_ID
    assert ctx.actor == "cli:squad"


def test_par_vazio_e_wire_pre_d2t_sem_contexto():
    assert TenantContext.from_wire("", "") is None


@pytest.mark.parametrize(
    "tenant_id,actor",
    [
        ("nao-e-uuid", "cli:squad"),
        # fora da forma canônica que o Rust emite (maiúsculas) — aceitar
        # quebraria o eco verbatim do D2t
        ("00000000-0000-0000-0000-00000000B4AA", "cli:squad"),
        (LOCAL_TENANT_ID, ""),          # tenant sem actor: caller quebrado
        ("", "cli:squad"),              # actor sem tenant: idem
        (LOCAL_TENANT_ID, "   "),       # actor só-espaço
    ],
)
def test_invalido_ou_parcial_e_fail_closed(tenant_id, actor):
    with pytest.raises(ValueError):
        TenantContext.from_wire(tenant_id, actor)


def test_contexto_e_imutavel():
    ctx = TenantContext.from_wire(LOCAL_TENANT_ID, "cli:squad")
    with pytest.raises(Exception):
        ctx.tenant_id = "outro"  # type: ignore[misc]


class _FakeRequest:
    """Só os campos que o caminho fail-closed toca — a recusa acontece ANTES
    de qualquer canal gRPC ser aberto, então o teste não precisa de rede."""

    task_id = "t-invalido"
    tenant_id = "nao-e-uuid"
    actor = "cli:squad"


def test_execute_task_recusa_tenant_invalido_com_um_evento_de_erro():
    # Sem pytest-asyncio no workspace (convenção existente): asyncio.run.
    import asyncio

    from btv_squad.server import SquadServicer

    async def coleta():
        servicer = SquadServicer(core_socket="/tmp/inexistente.sock")
        return [ev async for ev in servicer.ExecuteTask(_FakeRequest(), context=None)]

    eventos = asyncio.run(coleta())
    assert len(eventos) == 1, "recusa fail-closed é UM evento de erro e fim do stream"
    assert eventos[0].error.startswith("contexto de tenant inválido (fail-closed)")
    assert eventos[0].task_id == "t-invalido"
