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


def test_limiar_calibrado_deixa_maioria_forte_automatica():
    """PATCH ciclo completo: com o limiar fixo 0.7, TODO consenso de 3 agentes
    (pesos 0.9/0.6/0.5) escalava para humano por construção — o teto
    estrutural é 45%. Com 2 agentes o teto sobe e um vencedor forte passa
    sem humano: o limiar é calibrado pelo teto (0.6375), não um número
    mágico inalcançável."""
    engine = WeightedConsensusEngine()
    result = engine.reach_consensus(
        {
            "architect": Proposal(confidence=1.0),
            "developer": Proposal(confidence=0.5),
        },
        decision_type="architecture",
    )
    # arch: 0.9*1.0 / (0.9 + 0.3) = 0.75 ≥ 0.6375 (teto 0.75 * 0.85)
    assert result.consensus_strength > result.threshold_applied
    assert result.requires_human is False, "maioria forte de 2 agentes não precisa de humano"
    assert result.winner_confidence == 1.0, "confiança REAL do vencedor, não a share"


def test_limiar_calibrado_para_trio_segue_escalando():
    """Regressão do incidente real (arch 0.8/dev 0.85/aud 0.9 → 43%):
    a calibração NÃO libera o trio — o teto estrutural (62%) é atingível
    apenas com confiança perfeita; 43% continua abaixo do limiar calibrado
    (~53%), então o HITL do caso real continua disparando (honestidade:
    a mudança não fabrica consenso onde não há)."""
    engine = WeightedConsensusEngine()
    result = engine.reach_consensus(
        {
            "architect": Proposal(confidence=0.8),
            "developer": Proposal(confidence=0.85),
            "auditor": Proposal(confidence=0.9),
        },
        decision_type="architecture",
    )
    assert result.decision_maker == "architect"
    assert round(result.consensus_strength, 4) == round(3 / 7, 4), "0.4286… = winner_share"
    assert result.requires_human is True
    assert result.metric_definition == "winner_share"
    assert result.threshold_applied >= 0.3 and result.threshold_applied <= 0.7


def test_veto_do_auditor_escala_mesmo_com_consenso_forte():
    """PATCH ciclo completo: auditor com `approved: False` na proposta
    (conteúdo) força HITL mesmo quando a participação ponderada passaria
    sozinha — reprovação explícita nunca é atropelada por média alta."""
    engine = WeightedConsensusEngine()
    result = engine.reach_consensus(
        {
            "architect": Proposal(confidence=1.0),
            "auditor": Proposal(confidence=0.5, content={"approved": False}),
        },
        decision_type="architecture",
    )
    assert result.consensus_strength >= result.threshold_applied, "share passaria sozinha"
    assert result.requires_human is True, "veto do auditor escala independente da share"
    assert result.auditor_verdict is False


def test_auditor_aprovando_nao_veta():
    engine = WeightedConsensusEngine()
    result = engine.reach_consensus(
        {
            "architect": Proposal(confidence=1.0),
            "auditor": Proposal(confidence=0.5, content={"approved": True}),
        },
        decision_type="architecture",
    )
    assert result.auditor_verdict is True
    assert result.requires_human is False


def test_auditor_ausente_nao_fabrica_veredito():
    engine = WeightedConsensusEngine()
    result = engine.reach_consensus(
        {"architect": Proposal(confidence=1.0), "developer": Proposal(confidence=0.5)},
        decision_type="architecture",
    )
    assert result.auditor_verdict is None, "auditor ausente ≠ auditor aprovou"


def test_campos_de_auditoria_da_decisao():
    engine = WeightedConsensusEngine()
    result = engine.reach_consensus(
        {
            "architect": Proposal(confidence=0.8),
            "developer": Proposal(confidence=0.6),
        },
        decision_type="architecture",
    )
    assert result.proposal_confidences == {"architect": 0.8, "developer": 0.6}
    assert result.agent_weights == {"architect": 0.9, "developer": 0.6}
    assert result.winner_confidence == 0.8
    assert result.metric_definition == "winner_share"
    assert len(result.dissenting_opinions) == 1
