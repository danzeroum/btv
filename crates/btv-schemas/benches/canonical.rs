//! Bench criterion do hash canônico de cache (Fase 6 Onda 8) — o caminho quente
//! `request_hash` roda a cada chamada de LLM para a chave de cache. Produz um
//! baseline comparável (regressões de canonicalização/sha256 aparecem aqui).

use btv_schemas::request_hash;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_json::json;

fn bench_request_hash(c: &mut Criterion) {
    // Um histórico realista (mensagens + turnos) — o input do hash de cache.
    let messages = json!([
        {"role": "user", "content": "Explique o algoritmo de compaction de contexto"},
        {"role": "assistant", "content": "A compaction resume a conversa inteira quando o histórico se aproxima da janela, e uma nova época começa com o resumo como baseline."},
        {"role": "user", "content": "E como a época nova preserva o estado dos arquivos?"}
    ]);
    let temperature = json!(0.7);
    c.bench_function("request_hash", |b| {
        b.iter(|| request_hash(black_box(&messages), black_box(&temperature)))
    });
}

criterion_group!(benches, bench_request_hash);
criterion_main!(benches);
