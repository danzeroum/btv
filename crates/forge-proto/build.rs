//! Gera os stubs tonic a partir de `schemas/proto/*.proto`.
//!
//! Usa o `protoc` vendorizado (`protoc-bin-vendored`) em vez do binário de
//! sistema — evita exigir uma instalação de protobuf no ambiente de build.
//! Reexecuta quando qualquer `.proto` muda (`cargo:rerun-if-changed`).

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);

    let proto_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas/proto");
    let protos = ["promptforge.proto"];

    for proto in protos {
        println!("cargo:rerun-if-changed={proto_dir}/{proto}");
    }

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(&protos.map(|p| format!("{proto_dir}/{p}")), &[proto_dir])?;
    Ok(())
}
