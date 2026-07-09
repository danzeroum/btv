from typing import Any

from btv_squad.agents.base import BaseAgent


class _EchoAgent(BaseAgent):
    async def execute(self, task: dict[str, Any]) -> dict[str, Any]:
        return {"echo": task}


def test_agente_comeca_sem_memoria_nem_gateway():
    agent = _EchoAgent("echo")
    assert agent.memory is None
    assert agent.gateway is None
    assert agent.confidence_threshold == 0.7


def test_attach_memory_e_attach_gateway_injetam_dependencias():
    agent = _EchoAgent("echo")

    class _FakeMemory:
        def __init__(self):
            self.remembered = []

        def remember_decision(self, agent_type, entry):
            self.remembered.append((agent_type, entry))

    memory = _FakeMemory()
    agent.attach_memory(memory)
    agent.log_decision({"confidence": 0.9})

    assert len(memory.remembered) == 1
    assert memory.remembered[0][0] == "echo"


def test_validate_input_exige_description_nao_vazia():
    agent = _EchoAgent("echo")
    assert not agent.validate_input({})
    assert not agent.validate_input({"description": ""})
    assert agent.validate_input({"description": "fazer algo"})


def test_validate_confidence_respeita_o_limiar():
    agent = _EchoAgent("echo")
    assert not agent.validate_confidence(None)
    assert not agent.validate_confidence(0.5)
    assert agent.validate_confidence(0.7)
    assert agent.validate_confidence(0.9)


def test_log_decision_nao_quebra_sem_memoria_anexada():
    agent = _EchoAgent("echo")
    entry = agent.log_decision({"confidence": 0.5})
    assert entry["agent"] == "echo"
    assert entry["decision"] == {"confidence": 0.5}


def test_system_with_persona_prepende_a_persona_ao_base():
    agent = _EchoAgent("echo")
    base = "PROTOCOLO: responda em JSON."
    # Sem persona: devolve o base intacto.
    assert agent.system_with_persona(base) == base
    # Com persona: a voz/objetivo vem ANTES do protocolo (que fica preservado).
    agent.persona_prompt = "Você é o Redator. Escreva SEMPRE em voz ativa."
    combinado = agent.system_with_persona(base)
    assert combinado.startswith("Você é o Redator")
    assert base in combinado
