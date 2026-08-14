use anyhow::Result;
use std::fs;
use std::path::Path;
use uuid::Uuid;

const DEFAULT_ID_PATH: &str = "/var/lib/loghub-agent/agent-id";

pub fn load_or_create(path: Option<&str>) -> Result<String> {
    let id_path = path.unwrap_or(DEFAULT_ID_PATH);

    if let Ok(value) = fs::read_to_string(id_path) {
        let id = value.trim().to_string();
        if !id.is_empty() {
            return Ok(id);
        }
    }

    let id = Uuid::new_v4().to_string();

    if let Some(parent) = Path::new(id_path).parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(id_path, &id)?;
    Ok(id)
}
