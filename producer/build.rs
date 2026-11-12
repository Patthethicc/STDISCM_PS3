fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../proto/media_upload.proto");
    tonic_build::compile_protos("../proto/media_upload.proto")?;
    Ok(())
}
