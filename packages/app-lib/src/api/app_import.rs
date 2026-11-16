use crate::state::State;
use std::path::Path;

pub use crate::state::db_import::ImportSummary;

/// Copies instances, accounts and settings from another Modrinth App data
/// folder into this one. The other folder is only read.
#[tracing::instrument]
pub async fn import_modrinth_folder(
    folder: &Path,
) -> crate::Result<ImportSummary> {
    let state = State::get().await?;

    crate::state::db_import::import_folder(
        &state.pool,
        &state.directories.instances_dir(),
        folder,
    )
    .await
}
