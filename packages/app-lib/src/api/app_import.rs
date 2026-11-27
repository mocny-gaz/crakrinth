use crate::state::State;
use std::path::Path;

pub use crate::state::db_import::{ImportSummary, ImportableInstance};

const IMPORTED_FOLDERS_FILE: &str = "imported_modrinth_folders.txt";

/// Lists the instances in another Modrinth App data folder.
#[tracing::instrument]
pub async fn list_modrinth_instances(
    folder: &Path,
) -> crate::Result<Vec<ImportableInstance>> {
    crate::state::db_import::list_instances(folder).await
}

/// Copies instances, accounts and settings from another Modrinth App data
/// folder into this one. The other folder is only read. `selected` holds the
/// ids of the instances to import, or `None` for all of them.
#[tracing::instrument]
pub async fn import_modrinth_folder(
    folder: &Path,
    selected: Option<Vec<String>>,
) -> crate::Result<ImportSummary> {
    let state = State::get().await?;

    let summary = crate::state::db_import::import_folder(
        &state.pool,
        &state.directories.instances_dir(),
        folder,
        selected.as_deref(),
    )
    .await?;

    remember_imported_folder(&state.directories.settings_dir, folder);

    Ok(summary)
}

fn remember_imported_folder(settings_dir: &Path, folder: &Path) {
    let file = settings_dir.join(IMPORTED_FOLDERS_FILE);
    let line = folder.to_string_lossy().into_owned();
    let mut lines = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !lines.contains(&line) {
        lines.push(line);
        let _ = std::fs::write(&file, lines.join("\n"));
    }
}

pub(crate) fn modrinth_data_folders(settings_dir: &Path) -> Vec<std::path::PathBuf> {
    let mut folders = Vec::new();
    if let Some(default) = dirs::data_dir().map(|dir| dir.join("ModrinthApp")) {
        folders.push(default);
    }
    if let Ok(text) = std::fs::read_to_string(settings_dir.join(IMPORTED_FOLDERS_FILE)) {
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let path = std::path::PathBuf::from(line.trim());
            if !folders.contains(&path) {
                folders.push(path);
            }
        }
    }
    folders.retain(|folder| folder != settings_dir);
    folders
}
