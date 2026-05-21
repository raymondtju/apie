use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{Result, Workspace};

pub fn save_workspace(path: impl AsRef<Path>, workspace: &Workspace) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(workspace)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_workspace(path: impl AsRef<Path>) -> Result<Workspace> {
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}
pub fn default_workspace_path(base_dir: impl AsRef<Path>, workspace_id: &str) -> PathBuf {
    base_dir
        .as_ref()
        .join("gpui-api-client")
        .join("workspaces")
        .join(format!("{workspace_id}.json"))
}
