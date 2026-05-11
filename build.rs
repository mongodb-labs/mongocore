fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true) // For testing
        .compile_protos(&["proto/mongocore/v1/mongocore.proto"], &["proto"])?;
    Ok(())
}
