use std::io::Result;

fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    config.compile_protos(
        &[
            "proto-public-api/public_api_types.proto",
            "proto-public-api/public_api_up.proto",
            "proto-public-api/public_api_down.proto",
        ],
        &["proto-public-api/"],
    )?;
    Ok(())
}
