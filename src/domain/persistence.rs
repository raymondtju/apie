use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{Collection, CollectionItem, Result, Workspace};

/// Wrapper struct that lets us version the workspace file format so
/// future schema changes can be migrated transparently.
#[derive(Serialize, Deserialize)]
struct WorkspaceFile {
    version: u8,
    workspace: Workspace,
}

pub fn save_workspace(path: impl AsRef<Path>, workspace: &Workspace) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = WorkspaceFile {
        version: 2,
        workspace: workspace.clone(),
    };
    let json = serde_json::to_string_pretty(&file)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_workspace(path: impl AsRef<Path>) -> Result<Workspace> {
    let json = fs::read_to_string(path.as_ref())?;

    // Try versioned format first (v2+).
    if let Ok(file) = serde_json::from_str::<WorkspaceFile>(&json) {
        return Ok(file.workspace);
    }

    // Fallback: legacy pre-v2 format — Workspace with Vec<CollectionItem> at root.
    // Parse as the old shape, then wrap root items into a single "Default" collection.
    #[derive(Deserialize)]
    struct LegacyWorkspace {
        id: String,
        name: String,
        #[serde(default)]
        active_environment: String,
        #[serde(default)]
        environments: Vec<super::Environment>,
        items: Vec<CollectionItem>,
        #[serde(default)]
        expanded_folders: Vec<String>,
    }

    let legacy: LegacyWorkspace = serde_json::from_str(&json)?;
    let collection = Collection {
        id: "1".to_string(),
        name: "Default".to_string(),
        items: legacy.items,
    };

    Ok(Workspace {
        id: legacy.id,
        name: legacy.name,
        active_environment: legacy.active_environment,
        environments: legacy.environments,
        items: vec![collection],
        expanded_folders: legacy.expanded_folders,
        expanded_collections: Vec::new(),
    })
}

pub fn default_workspace_path(base_dir: impl AsRef<Path>, workspace_id: &str) -> PathBuf {
    base_dir
        .as_ref()
        .join("gpui-api-client")
        .join("workspaces")
        .join(format!("{workspace_id}.json"))
}
