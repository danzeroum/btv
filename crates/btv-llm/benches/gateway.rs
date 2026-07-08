//! Bench criterion do caminho do gateway (Fase 6 Onda 8): mede o overhead de uma
//! geração através do `Generator` **sem key real** (o `ScriptedGenerator`) — a
//! serialização/agregação/streaming do nosso lado, isolada da latência de rede
//! do provider. É o mesmo generator que o load-test do k6 usa.

use btv_llm::chat::GenerateRequest;
use btv_llm::{Generator, ScriptedGenerator};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn req() -> GenerateRequest {
    GenerateRequest {
        model: "scripted".into(),
        system: "Você é um agente de coding.".into(),
        messages: vec![],
        tools: vec![],
        max_tokens: 256,
        temperature: Some(0.7),
    }
}

fn bench_scripted_generate(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let generator = ScriptedGenerator::echo("resposta do gateway, sem key real");
    c.bench_function("scripted_generate", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut sink = |_: &str| {};
                generator
                    .generate(black_box(req()), &mut sink)
                    .await
                    .unwrap()
            })
        })
    });
}

criterion_group!(benches, bench_scripted_generate);
criterion_main!(benches);
