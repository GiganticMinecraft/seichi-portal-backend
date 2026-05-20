use std::{fs, path::PathBuf};

const OPENAPI_PATH: &str = "docs/openapi.json";

fn main() -> anyhow::Result<()> {
    let path = repository_root().join(OPENAPI_PATH);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let openapi = entrypoint::openapi::openapi();
    let mut json = serde_json::to_string_pretty(&openapi)?;
    json.push('\n');

    fs::write(path, json)?;

    Ok(())
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|server_dir| server_dir.parent())
        .expect("entrypoint crate must be under server/entrypoint")
        .to_path_buf()
}
