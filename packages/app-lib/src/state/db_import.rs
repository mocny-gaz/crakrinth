use super::dirs::INSTANCES_FOLDER_NAME;
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection, SqlitePoolOptions};
use sqlx::{ConnectOptions, Connection, Pool, Sqlite};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Tables copied row by row. Keys are text ids, so rows never collide with
/// different data; rows that already exist are left alone.
const COPIED_TABLES: &[&str] = &[
    "attached_world_data",
    "instance_content_sets",
    "instance_files",
    "instance_content_entries",
    "instance_content_locks",
    "instance_content_set_remote_refs",
    "instance_content_set_sync_state",
    "instance_content_update_checks",
    "instance_groups",
    "instance_launch_overrides",
    "instance_links",
    "instance_quarantines",
    "join_log",
    "custom_minecraft_skin_textures",
    "custom_minecraft_skins",
    "minecraft_users",
    "modrinth_users",
];

/// Settings that describe this install, not the user's preferences.
const KEPT_SETTINGS: &[&str] = &[
    "id",
    "custom_dir",
    "prev_custom_dir",
    "migrated",
    "version",
    "auto_download_updates",
    "skipped_update",
    "pending_update_toast_for_version",
];

#[derive(Serialize, Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub instances: usize,
    pub accounts: usize,
}

fn error(message: String) -> crate::Error {
    crate::ErrorKind::OtherError(message).as_error()
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Columns that exist in both this database and the attached `src` one.
async fn common_columns(
    conn: &mut SqliteConnection,
    table: &str,
) -> crate::Result<Vec<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT name FROM pragma_table_info(?, 'main') WHERE name IN (SELECT name FROM pragma_table_info(?, 'src'))",
    )
    .bind(table)
    .bind(table)
    .fetch_all(&mut *conn)
    .await?)
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct ImportableInstance {
    pub id: String,
    pub name: String,
    pub path: String,
}

async fn open_source_copy(
    folder: &Path,
) -> crate::Result<(tempfile::TempDir, String, Pool<Sqlite>)> {
    let source_db = folder.join("app.db");
    if !source_db.is_file() {
        return Err(error(format!(
            "{} does not contain an app.db. Choose the folder that holds your Modrinth App data",
            folder.display()
        )));
    }

    let work = tempfile::tempdir()?;
    let copy_path = work.path().join("source.db");
    let copy_str = copy_path.to_string_lossy().into_owned();
    {
        let mut conn = SqliteConnectOptions::new()
            .filename(&source_db)
            .read_only(true)
            .connect()
            .await?;
        sqlx::query("VACUUM INTO ?")
            .bind(&copy_str)
            .execute(&mut conn)
            .await?;
    }
    let source = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&copy_path))
        .await?;
    super::db_downgrade::prepare_copy(&source).await?;
    sqlx::migrate!().run(&source).await?;
    Ok((work, copy_str, source))
}

pub(crate) async fn list_instances(
    folder: &Path,
) -> crate::Result<Vec<ImportableInstance>> {
    let (_work, _copy, source) = open_source_copy(folder).await?;
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, path FROM instances ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(&source)
    .await?;
    source.close().await;
    Ok(rows
        .into_iter()
        .map(|(id, name, path)| ImportableInstance { id, name, path })
        .collect())
}

/// Copies instances, accounts and settings out of another Modrinth App data
/// folder. Nothing in that folder is changed or moved. The current database
/// is backed up first and the database part is a single transaction.
pub(crate) async fn import_folder(
    pool: &Pool<Sqlite>,
    instances_dir: &Path,
    folder: &Path,
    selected: Option<&[String]>,
) -> crate::Result<ImportSummary> {
    let (_work, copy_str, source) = open_source_copy(folder).await?;

    if let Some(selected) = selected {
        sqlx::query("PRAGMA foreign_keys = ON").execute(&source).await?;
        let mut delete = sqlx::QueryBuilder::<Sqlite>::new(
            "DELETE FROM instances WHERE id NOT IN (",
        );
        let mut ids = delete.separated(", ");
        for id in selected {
            ids.push_bind(id.clone());
        }
        ids.push_unseparated(")");
        delete.build().execute(&source).await?;
    }

    let custom_dir = sqlx::query_scalar::<_, Option<String>>(
        "SELECT custom_dir FROM settings",
    )
    .fetch_optional(&source)
    .await?
    .flatten();
    source.close().await;
    let source_instances = custom_dir
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| folder.to_path_buf(), PathBuf::from)
        .join(INSTANCES_FOLDER_NAME);

    let backup_dir = super::db_backup::app_db_backup_dir()?;
    std::fs::create_dir_all(&backup_dir)?;
    let backup = backup_dir.join(format!(
        "app-db-before-import-{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis())
    ));
    sqlx::query("VACUUM INTO ?")
        .bind(backup.to_string_lossy().into_owned())
        .execute(pool)
        .await?;

    let mut conn = pool.acquire().await?;
    sqlx::query("ATTACH DATABASE ? AS src")
        .bind(&copy_str)
        .execute(&mut *conn)
        .await?;
    let result = merge(&mut conn, instances_dir, &source_instances).await;
    sqlx::query("DETACH DATABASE src")
        .execute(&mut *conn)
        .await?;

    result
}

async fn merge(
    conn: &mut SqliteConnection,
    instances_dir: &Path,
    source_instances: &Path,
) -> crate::Result<ImportSummary> {
    let new_instances = sqlx::query_as::<_, (String, String)>(
        "SELECT id, path FROM src.instances WHERE id NOT IN (SELECT id FROM main.instances)",
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut taken =
        sqlx::query_scalar::<_, String>("SELECT path FROM main.instances")
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect::<BTreeSet<_>>();

    // Pick a free folder name for every new instance and copy its folder.
    let mut plan = Vec::new();
    for (id, path) in new_instances {
        let mut target = path.clone();
        let mut suffix = 1;
        while taken.contains(&target) || instances_dir.join(&target).exists() {
            suffix += 1;
            target = if suffix == 2 {
                format!("{path}-imported")
            } else {
                format!("{path}-imported-{}", suffix - 1)
            };
        }
        taken.insert(target.clone());

        let from = source_instances.join(&path);
        let to = instances_dir.join(&target);
        if from.is_dir() {
            tokio::task::spawn_blocking(move || copy_dir(&from, &to))
                .await
                .map_err(|e| error(e.to_string()))??;
        }
        plan.push((id, target));
    }

    let accounts = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM src.minecraft_users WHERE uuid NOT IN (SELECT uuid FROM main.minecraft_users)",
    )
    .fetch_one(&mut *conn)
    .await? as usize;

    let mut tx = conn.begin().await?;
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *tx)
        .await?;

    let columns = common_columns(&mut tx, "instances").await?;
    let names = columns
        .iter()
        .map(|column| quote(column))
        .collect::<Vec<_>>()
        .join(", ");
    let values = columns
        .iter()
        .map(|column| {
            if column == "path" {
                "?".into()
            } else {
                quote(column)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    for (id, path) in &plan {
        sqlx::query(&format!(
            "INSERT INTO main.instances ({names}) SELECT {values} FROM src.instances WHERE id = ?"
        ))
        .bind(path)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }

    for table in COPIED_TABLES {
        let columns = common_columns(&mut tx, table).await?;
        if columns.is_empty() {
            continue;
        }
        let names = columns
            .iter()
            .map(|column| quote(column))
            .collect::<Vec<_>>()
            .join(", ");
        let values = columns
            .iter()
            .map(|column| {
                if *table == "minecraft_users" && column == "active" {
                    // Only one account may be the active one.
                    "CASE WHEN EXISTS (SELECT 1 FROM main.minecraft_users WHERE active) THEN 0 ELSE active END".to_string()
                } else {
                    quote(column)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let table = quote(table);
        sqlx::query(&format!(
            "INSERT OR IGNORE INTO main.{table} ({names}) SELECT {values} FROM src.{table}"
        ))
        .execute(&mut *tx)
        .await?;
    }

    let settings = common_columns(&mut tx, "settings")
        .await?
        .into_iter()
        .filter(|column| !KEPT_SETTINGS.contains(&column.as_str()))
        .map(|column| {
            let column = quote(&column);
            format!("{column} = (SELECT {column} FROM src.settings LIMIT 1)")
        })
        .collect::<Vec<_>>()
        .join(", ");
    if !settings.is_empty() {
        sqlx::query(&format!(
            "UPDATE main.settings SET {settings} WHERE EXISTS (SELECT 1 FROM src.settings)"
        ))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(ImportSummary {
        instances: plan.len(),
        accounts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn migrated(path: &Path) -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    async fn add_instance(pool: &Pool<Sqlite>, id: &str, path: &str) {
        sqlx::query("INSERT INTO instances (id, path, install_stage, launcher_feature_version, name, created, modified) VALUES (?, ?, 'installed', 'none', ?, 0, 0)")
            .bind(id)
            .bind(path)
            .bind(path)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn imports_instances_accounts_and_settings_once() {
        let root = tempfile::tempdir().unwrap();
        // SAFETY: only this test in this module touches the environment.
        unsafe {
            std::env::set_var(
                "THESEUS_DB_BACKUP_DIR",
                root.path().join("backups"),
            )
        };

        let source_dir = root.path().join("old");
        std::fs::create_dir_all(source_dir.join("profiles/Pack/mods")).unwrap();
        std::fs::write(source_dir.join("profiles/Pack/mods/a.jar"), b"jar")
            .unwrap();
        let source = migrated(&source_dir.join("app.db")).await;
        add_instance(&source, "id-1", "Pack").await;
        add_instance(&source, "id-2", "Other").await;
        sqlx::query("INSERT INTO minecraft_users (uuid, active, username, access_token, refresh_token, expires) VALUES ('u1', 1, 'Steve', 't', 'r', 0)")
            .execute(&source)
            .await
            .unwrap();
        sqlx::query("UPDATE settings SET theme = 'light', onboarded = 1")
            .execute(&source)
            .await
            .unwrap();
        source.close().await;

        let dest_dir = root.path().join("new");
        std::fs::create_dir_all(dest_dir.join("profiles/Pack")).unwrap();
        let dest = migrated(&dest_dir.join("app.db")).await;
        add_instance(&dest, "mine", "Pack").await;
        sqlx::query("INSERT INTO minecraft_users (uuid, active, username, access_token, refresh_token, expires) VALUES ('u0', 1, 'Alex', 't', 'r', 0)")
            .execute(&dest)
            .await
            .unwrap();
        let instances_dir = dest_dir.join("profiles");

        let summary = import_folder(&dest, &instances_dir, &source_dir, None)
            .await
            .unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                instances: 2,
                accounts: 1
            }
        );

        // Name clash gets a new folder, everything is copied, source is untouched.
        assert!(instances_dir.join("Pack-imported/mods/a.jar").is_file());
        assert!(source_dir.join("profiles/Pack/mods/a.jar").is_file());
        let paths = sqlx::query_scalar::<_, String>(
            "SELECT path FROM instances ORDER BY path",
        )
        .fetch_all(&dest)
        .await
        .unwrap();
        assert_eq!(paths, ["Other", "Pack", "Pack-imported"]);

        // Existing active account wins; settings carried over.
        let active = sqlx::query_scalar::<_, String>(
            "SELECT username FROM minecraft_users WHERE active",
        )
        .fetch_all(&dest)
        .await
        .unwrap();
        assert_eq!(active, ["Alex"]);
        let theme =
            sqlx::query_scalar::<_, String>("SELECT theme FROM settings")
                .fetch_one(&dest)
                .await
                .unwrap();
        assert_eq!(theme, "light");

        // Running it again adds nothing.
        let again = import_folder(&dest, &instances_dir, &source_dir, None)
            .await
            .unwrap();
        assert_eq!(
            again,
            ImportSummary {
                instances: 0,
                accounts: 0
            }
        );
        assert!(
            std::fs::read_dir(root.path().join("backups"))
                .unwrap()
                .count()
                >= 1
        );
    }
}
