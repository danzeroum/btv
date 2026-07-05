import asyncio

import pytest

from forge_squad.permission import PermissionDecision, PermissionRequest, ScriptedPermissionClient


def test_scripted_client_devolve_decisoes_na_ordem():
    client = ScriptedPermissionClient(
        [PermissionDecision(approved=True), PermissionDecision(approved=False, operator_note="risco alto")]
    )
    request = PermissionRequest(tool="approve_plan", scope="architect", reason="plano crítico", confidence=0.4)

    first = asyncio.run(client.request_permission(request))
    second = asyncio.run(client.request_permission(request))

    assert first.approved is True
    assert second.approved is False
    assert second.operator_note == "risco alto"
    assert client.requests == [request, request]


def test_scripted_client_esgotado_levanta_assertion_error():
    client = ScriptedPermissionClient([PermissionDecision(approved=True)])
    request = PermissionRequest(tool="x", scope="y", reason="z", confidence=0.5)

    asyncio.run(client.request_permission(request))
    with pytest.raises(AssertionError):
        asyncio.run(client.request_permission(request))
