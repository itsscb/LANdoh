use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_file = vec![
        "./proto/proto.proto",
        // "./proto/pb.proto",
    ];
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .type_attribute("File", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute("Game", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(
            "RegistryKey",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .type_attribute("Path", "#[derive(serde::Serialize, serde::Deserialize)]")
        .enum_attribute(
            "Path.location",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .enum_attribute(
            "File.file_payload",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .type_attribute(
            "FileMetaData",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .file_descriptor_set_path(out_dir.join("pb_descriptor.bin"))
        // .out_dir("./src")
        .compile(&proto_file, &["proto"])?;

    tauri_build::build();
    Ok(())
}
