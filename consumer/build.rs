fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)   // set false if you only ship the server
        .compile_protos(&["../proto/media_upload.proto"], &["../proto"])?;
    println!("cargo:rerun-if-changed=../proto/media_upload.proto");
    Ok(())
}