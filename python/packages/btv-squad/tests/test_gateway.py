import asyncio

import pytest

from btv_squad.gateway import LlmRequest, LlmResponse, ScriptedGatewayClient


def test_scripted_gateway_devolve_respostas_na_ordem():
    gateway = ScriptedGatewayClient(
        [LlmResponse(text="primeira"), LlmResponse(text="segunda")]
    )
    req = LlmRequest(model="claude-sonnet-5", requester="architect")

    first = asyncio.run(gateway.generate(req))
    second = asyncio.run(gateway.generate(req))

    assert first.text == "primeira"
    assert second.text == "segunda"
    assert gateway.requests == [req, req]


def test_scripted_gateway_esgotado_levanta_assertion_error():
    gateway = ScriptedGatewayClient([LlmResponse(text="única")])
    req = LlmRequest(model="claude-sonnet-5", requester="architect")

    asyncio.run(gateway.generate(req))
    with pytest.raises(AssertionError):
        asyncio.run(gateway.generate(req))
