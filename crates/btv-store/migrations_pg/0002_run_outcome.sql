-- PATCH ciclo completo: veredito final da run, aditivo à tabela `runs`.
-- O watcher Rust deriva `resultado`/`motivo` do evento `run_result` do
-- stream do squad (aprovada/reprovada/incompleta) — sem isto, uma run
-- reprovada pela auditoria aparecia como "concluída" sem artefato.
ALTER TABLE runs ADD COLUMN resultado TEXT;
ALTER TABLE runs ADD COLUMN motivo TEXT;