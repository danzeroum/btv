import asyncio
import json

from btv_squad.agents.designer import DesignerAgent
from btv_squad.gateway import LlmResponse, ScriptedGatewayClient


def _design_payload(**overrides):
    payload = {
        "pattern": "fluent",
        "components": ["header", "carrinho", "checkout"],
        "colors": {"primary": "#0a5", "secondary": "#f60", "background": "#fff"},
        "typography": {"font": "Inter", "sizes": {"h1": "2.5rem", "body": "1rem"}},
        "responsive": True,
        "accessibility": "WCAG 2.1 AA",
        "confidence": 0.8,
        "notes": "Layout otimizado para conversão de checkout",
    }
    payload.update(overrides)
    return payload


def test_execute_deriva_design_real_do_gateway():
    agent = DesignerAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(_design_payload()))]))

    result = asyncio.run(agent.execute({"description": "página de checkout de e-commerce"}))

    assert result["success"] is True
    assert result["pattern"] == "fluent"
    assert result["components"] == ["header", "carrinho", "checkout"]
    assert result["colors"]["primary"] == "#0a5"
    assert result["confidence"] == 0.8


def test_padrao_nao_suportado_cai_para_material():
    agent = DesignerAgent()
    agent.attach_gateway(
        ScriptedGatewayClient([LlmResponse(text=json.dumps(_design_payload(pattern="bootstrap-legado")))])
    )

    result = asyncio.run(agent.execute({"description": "dashboard interno"}))

    assert result["pattern"] == "material"


def test_dois_pedidos_diferentes_produzem_designs_diferentes():
    agent = DesignerAgent()
    agent.attach_gateway(
        ScriptedGatewayClient(
            [
                LlmResponse(text=json.dumps(_design_payload(pattern="material", components=["mapa"]))),
                LlmResponse(text=json.dumps(_design_payload(pattern="carbon", components=["tabela", "filtro"]))),
            ]
        )
    )

    result_a = asyncio.run(agent.execute({"description": "app de mapas"}))
    result_b = asyncio.run(agent.execute({"description": "painel administrativo"}))

    assert result_a["pattern"] == "material"
    assert result_a["components"] == ["mapa"]
    assert result_b["pattern"] == "carbon"
    assert result_b["components"] == ["tabela", "filtro"]


def test_execute_sem_gateway_levanta_erro_claro():
    agent = DesignerAgent()
    try:
        asyncio.run(agent.execute({"description": "tarefa qualquer"}))
        assert False, "deveria ter levantado RuntimeError"
    except RuntimeError as exc:
        assert "attach_gateway" in str(exc)


def test_resposta_sem_json_cai_no_fallback_honesto():
    agent = DesignerAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text="não consigo desenhar isso.")]))

    result = asyncio.run(agent.execute({"description": "tarefa"}))

    assert result["components"] == []
    assert result["confidence"] == 0.0
    assert result["pattern"] == "material"  # default determinístico da guarda
