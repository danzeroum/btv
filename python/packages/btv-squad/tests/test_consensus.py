from btv_squad.consensus import Proposal, WeightedConsensusEngine


def test_especialista_vence_no_seu_dominio():
    engine = WeightedConsensusEngine()
    result = engine.reach_consensus(
        {
            "auditor": Proposal(confidence=0.8),
            "designer": Proposal(confidence=0.9),
        },
        decision_type="security",
    )
    # auditor: 0.95*0.8 = 0.76 > designer: 0.5*0.9 = 0.45 (peso default)
    assert result.decision_maker == "auditor"
    assert len(result.dissenting_opinions) == 1
    assert result.dissenting_opinions[0].agent == "designer"


def test_consenso_fraco_escala_para_humano():
    engine = WeightedConsensusEngine()
    result = engine.reach_consensus(
        {
            "architect": Proposal(confidence=0.6),
            "developer": Proposal(confidence=0.6),
            "auditor": Proposal(confidence=0.6),
        },
        decision_type="architecture",
    )
    assert result.requires_human  # três votos parelhos: nenhum domina


def test_sem_propostas_consenso_zero():
    result = WeightedConsensusEngine().reach_consensus({}, decision_type="architecture")
    assert result.decision is None
    assert result.consensus_strength == 0.0
    assert result.requires_human
