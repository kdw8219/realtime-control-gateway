fn main() -> Result<(), Box<dyn std::error::Error>> {
   tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(
            &["proto/connection/control.proto", "proto/connection/signaling.proto"],
            &["proto/connection/"],
        )?;
   Ok(())
}