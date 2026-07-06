//! Bench criterion do caminho de contexto/épocas (Fase 6 Onda 8): `estimate_tokens`
//! e `needs_compaction` rodam a cada turno do loop de agente para decidir se a
//! conversa precisa ser compactada. Baseline comparável do hot path.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use forge_core::{estimate_tokens, CompactionPolicy};
use forge_llm::chat::ChatMessage;
use forge_llm::tier_from_id;

fn historico(n: usize) -> Vec<ChatMessage> {
    (0..n)
        .map(|i| {
            ChatMessage::user_text(format!(
                "mensagem {i}: conteúdo de tamanho médio para exercitar a heurística de tokens no caminho quente do loop"
            ))
        })
        .collect()
}

fn bench_estimate_tokens(c: &mut Criterion) {
    let msgs = historico(200);
    c.bench_function("estimate_tokens_200", |b| {
        b.iter(|| estimate_tokens(black_box(&msgs)))
    });
}

fn bench_needs_compaction(c: &mut Criterion) {
    let policy = CompactionPolicy::for_tier(tier_from_id("claude-sonnet-5"), 200_000);
    let msgs = historico(200);
    c.bench_function("needs_compaction_200", |b| {
        b.iter(|| policy.needs_compaction(black_box(&msgs)))
    });
}

criterion_group!(benches, bench_estimate_tokens, bench_needs_compaction);
criterion_main!(benches);
