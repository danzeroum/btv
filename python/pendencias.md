
# Rename completo do motor: Forge → BuildToValue/btv

Pedido explícito do usuário após a entrega das 6 ondas. Esquema aplicado:
`btv` como identificador técnico (crates `btv-*`, pacotes Python `btv_*`,
binário `btv`, protos `btv.*.v1`, env vars `BTV_*`, diretório de dados
`.btv/`, `btv.toml`) e **BuildToValue** na prosa/UI. 284 arquivos + 19
diretórios/arquivos movidos; stubs gRPC Python regenerados (o descritor
serializado embute o package — sed não basta); `uv.lock`/`Cargo.lock`
regenerados; screenshots do console dev regeneradas (strings mudaram).

- **[decisão] Exceções preservadas de propósito:** `PromptForge` (nome
  próprio do componente de prompts, herdado do prompte — package
  `btv.promptforge.v1`, serviço `PromptForgeService`); `forgetting.py`
  ("forget" contém "forge" — palavra inglesa, não marca); documentos
  históricos em `docs/` (ADRs, PLANO-*, DECISOES, handoffs de design,
  roadmap-forge.html) ficam como registro com o nome antigo — reescrever
  histórico falsificaria as decisões; este arquivo idem (as seções acima
  citam os nomes da época).
- **[achados reais do sed, corrigidos]** duas corrupções por substring:
  `.forgetting` → `.btvtting` (regra `.forge`) e `IntelligentForgetting` →
  `IntelligentBuildToValuetting` (regra `Forge`) — pegas por teste e por
  varredura de colagem (`BuildToValue` grudado em palavra), restauradas.
- **[migração]** instalações existentes: `mv .forge .btv` + renomear env
  vars `FORGE_*`→`BTV_*` (nota no README). Sem código de migração
  automática — local-first, decisão documentada.
- **[fixes de UI da auditoria]** os 3 vazamentos de nome de motor na UI do
  produto foram reescritos (erro da galeria sem citar comando; auditoria do
  Designer "ledger da plataforma"; nota do A4 sem nome de crate).
