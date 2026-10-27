use std::path::Path;

use serde_json::Value;

use super::RpcError;

pub fn not_claude() -> RpcError {
    RpcError::invalid("só uma sessão do claude tem retrato")
}

pub fn resolve_portrait(image: &Value, plugin_dir: &Path, data_dir: &Path) -> Result<String, RpcError> {
    let path = image
        .as_str()
        .filter(|p| p.to_lowercase().ends_with(".png") && Path::new(p).is_file())
        .ok_or_else(|| RpcError::invalid("\"image\": o caminho de um .png que existe"))?;
    let real = std::fs::canonicalize(path)
        .map_err(|_| RpcError::invalid("\"image\": o caminho de um .png que existe"))?;
    let inside = |dir: &Path| {
        std::fs::canonicalize(dir).is_ok_and(|d| real.to_string_lossy().starts_with(&format!("{}/", d.to_string_lossy())))
    };
    if !inside(plugin_dir) && !inside(data_dir) {
        return Err(RpcError::forbidden("\"image\" tem que estar na pasta do plugin ou na de dados dele"));
    }
    Ok(real.to_string_lossy().into_owned())
}
