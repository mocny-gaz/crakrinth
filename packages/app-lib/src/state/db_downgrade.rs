use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

type Schema = BTreeMap<String, BTreeSet<String>>;

/// What a downgrade removed, so a later upgrade can put the data back.
#[derive(Serialize, Deserialize, Default)]
struct Removed {
    tables: Vec<String>,
    columns: Vec<(String, String)>,
}

fn error(message: String) -> crate::Error {
    crate::ErrorKind::OtherError(message).as_error()
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Path without extension of the backup made when going from `from` down to `to`.
fn backup_stem(from: &str, to: &str) -> crate::Result<String> {
    let name = format!(
        "app-db-downgrade-from-{}-to-{}",
        super::db_backup::sanitize_version_for_filename(from),
        super::db_backup::sanitize_version_for_filename(to)
    );
    let dir = super::db_backup::app_db_backup_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(name).to_string_lossy().into_owned())
}

async fn has_table(pool: &Pool<Sqlite>, name: &str) -> crate::Result<bool> {
    Ok(sqlx::query(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?
    .is_some())
}

async fn columns(
    conn: &mut SqliteConnection,
    table: &str,
    schema: &str,
) -> crate::Result<BTreeSet<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT name FROM pragma_table_info(?, ?)",
    )
    .bind(table)
    .bind(schema)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect())
}

async fn schema(conn: &mut SqliteConnection) -> crate::Result<Schema> {
    let tables = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations'",
    )
    .fetch_all(&mut *conn)
    .await?;

    let mut schema = Schema::new();
    for table in tables {
        let table_columns = columns(conn, &table, "main").await?;
        schema.insert(table, table_columns);
    }
    Ok(schema)
}

/// Triggers, views and indexes created by SQL, as (kind, name).
async fn objects(
    conn: &mut SqliteConnection,
) -> crate::Result<BTreeSet<(String, String)>> {
    Ok(sqlx::query_as::<_, (String, String)>(
        "SELECT type, name FROM sqlite_master WHERE type IN ('trigger', 'view', 'index') AND sql IS NOT NULL",
    )
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect())
}

/// An in-memory database holding exactly the schema this version expects.
async fn reference_pool() -> crate::Result<Pool<Sqlite>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(SqliteConnectOptions::new().in_memory(true))
        .await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

/// Run before migrations. If the database was migrated by a newer version,
/// backs it up and strips everything this version does not know about.
/// Returns the app version that last used the database.
pub(crate) async fn prepare(
    pool: &Pool<Sqlite>,
) -> crate::Result<Option<String>> {
    let stored = if has_table(pool, "app_metadata").await? {
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM app_metadata WHERE key = 'app_version'",
        )
        .fetch_optional(pool)
        .await?
    } else {
        None
    };

    if !has_table(pool, "_sqlx_migrations").await? {
        return Ok(stored);
    }

    let known = sqlx::migrate!()
        .migrations
        .iter()
        .map(|migration| migration.version)
        .collect::<BTreeSet<_>>();
    let unknown =
        sqlx::query_scalar::<_, i64>("SELECT version FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?
            .into_iter()
            .filter(|version| !known.contains(version))
            .collect::<Vec<_>>();

    if !unknown.is_empty() {
        downgrade(pool, stored.as_deref().unwrap_or("unknown"), &unknown)
            .await?;
    }

    Ok(stored)
}

async fn downgrade(
    pool: &Pool<Sqlite>,
    from: &str,
    unknown: &[i64],
) -> crate::Result<()> {
    tracing::warn!(
        "App database was migrated by version {from}, downgrading it to {APP_VERSION}"
    );

    let stem = backup_stem(from, APP_VERSION)?;
    let snapshot = format!("{stem}.db");
    let _ = std::fs::remove_file(&snapshot);
    sqlx::query("VACUUM INTO ?")
        .bind(&snapshot)
        .execute(pool)
        .await?;

    let reference = reference_pool().await?;
    let wanted = schema(&mut *reference.acquire().await?).await?;
    let wanted_objects = objects(&mut *reference.acquire().await?).await?;
    let have = schema(&mut *pool.acquire().await?).await?;
    let mut extra_objects = objects(&mut *pool.acquire().await?)
        .await?
        .into_iter()
        .filter(|object| !wanted_objects.contains(object))
        .collect::<Vec<_>>();
    extra_objects.sort_by_key(|(kind, _)| match kind.as_str() {
        "trigger" => 0,
        "view" => 1,
        _ => 2,
    });

    let mut tx = pool.begin().await?;
    let mut removed = Removed::default();

    for (kind, name) in extra_objects {
        let sql = format!("DROP {kind} IF EXISTS {}", quote(&name));
        if let Err(err) = sqlx::query(&sql).execute(&mut *tx).await {
            tracing::warn!("Could not run `{sql}`: {err}");
        }
    }

    for (table, table_columns) in &have {
        match wanted.get(table) {
            None => {
                let sql = format!("DROP TABLE {}", quote(table));
                match sqlx::query(&sql).execute(&mut *tx).await {
                    Ok(_) => removed.tables.push(table.clone()),
                    Err(err) => tracing::warn!("Could not run `{sql}`: {err}"),
                }
            }
            Some(wanted_columns) => {
                for column in table_columns.difference(wanted_columns) {
                    let sql = format!(
                        "ALTER TABLE {} DROP COLUMN {}",
                        quote(table),
                        quote(column)
                    );
                    match sqlx::query(&sql).execute(&mut *tx).await {
                        Ok(_) => removed
                            .columns
                            .push((table.clone(), column.clone())),
                        Err(err) => {
                            tracing::warn!("Could not run `{sql}`: {err}")
                        }
                    }
                }
            }
        }
    }

    let now = schema(&mut tx).await?;
    for (table, wanted_columns) in &wanted {
        if let Some(missing) = wanted_columns
            .iter()
            .find(|column| !now.get(table).is_some_and(|c| c.contains(*column)))
        {
            return Err(error(format!(
                "Cannot downgrade the app database to version {APP_VERSION}: {table}.{missing} is missing. Nothing was changed. A backup is at {snapshot}"
            )));
        }
    }

    for version in unknown {
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
            .bind(version)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    std::fs::write(
        format!("{stem}.json"),
        serde_json::to_string(&removed).map_err(|e| error(e.to_string()))?,
    )?;

    Ok(())
}

/// Run after migrations. When upgrading back to a version this database was
/// downgraded from, puts the data that the downgrade removed back in place.
/// With no such backup the upgrade is just the normal migration.
pub(crate) async fn restore(pool: &Pool<Sqlite>, previous: Option<&str>) {
    let Some(previous) = previous else { return };
    if previous == APP_VERSION {
        return;
    }

    if let Err(err) = restore_from(pool, APP_VERSION, previous).await {
        tracing::warn!("Could not restore data removed by a downgrade: {err}");
    }
}

async fn restore_from(
    pool: &Pool<Sqlite>,
    current: &str,
    previous: &str,
) -> crate::Result<()> {
    let stem = backup_stem(current, previous)?;
    let (snapshot, journal) = (format!("{stem}.db"), format!("{stem}.json"));
    let Ok(raw) = std::fs::read_to_string(&journal) else {
        return Ok(());
    };
    if !Path::new(&snapshot).exists() {
        return Ok(());
    }
    let removed = serde_json::from_str::<Removed>(&raw)
        .map_err(|e| error(e.to_string()))?;

    let mut conn = pool.acquire().await?;
    sqlx::query("ATTACH DATABASE ? AS backup")
        .bind(&snapshot)
        .execute(&mut *conn)
        .await?;
    let result = copy_back(&mut conn, &removed).await;
    sqlx::query("DETACH DATABASE backup")
        .execute(&mut *conn)
        .await?;
    result?;

    std::fs::remove_file(journal)?;
    Ok(())
}

async fn copy_back(
    conn: &mut SqliteConnection,
    removed: &Removed,
) -> crate::Result<()> {
    for table in &removed.tables {
        let common = columns(conn, table, "main")
            .await?
            .intersection(&columns(conn, table, "backup").await?)
            .map(|column| quote(column))
            .collect::<Vec<_>>()
            .join(", ");
        if common.is_empty() {
            continue;
        }
        let t = quote(table);
        sqlx::query(&format!(
            "INSERT OR IGNORE INTO main.{t} ({common}) SELECT {common} FROM backup.{t}"
        ))
        .execute(&mut *conn)
        .await?;
    }

    for (table, column) in &removed.columns {
        let in_main = columns(conn, table, "main").await?;
        let in_backup = columns(conn, table, "backup").await?;
        if !in_main.contains(column) || !in_backup.contains(column) {
            continue;
        }

        let keys = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info(?, 'main') WHERE pk > 0 ORDER BY pk",
        )
        .bind(table)
        .fetch_all(&mut *conn)
        .await?;
        let (t, c) = (quote(table), quote(column));
        let on = if keys.is_empty() {
            format!("b.rowid = {t}.rowid")
        } else {
            keys.iter()
                .map(|key| format!("b.{0} = {t}.{0}", quote(key)))
                .collect::<Vec<_>>()
                .join(" AND ")
        };
        sqlx::query(&format!(
            "UPDATE main.{t} SET {c} = (SELECT b.{c} FROM backup.{t} AS b WHERE {on}) WHERE EXISTS (SELECT 1 FROM backup.{t} AS b WHERE {on})"
        ))
        .execute(&mut *conn)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn value(pool: &Pool<Sqlite>, sql: &str) -> Option<String> {
        sqlx::query_scalar::<_, Option<String>>(sql)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn pretend_newer_version(pool: &Pool<Sqlite>) {
        for sql in [
            "ALTER TABLE settings ADD COLUMN future_col TEXT",
            "CREATE TABLE future_table (id INTEGER PRIMARY KEY, v TEXT)",
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (99999999999999, 'future', 1, x'00', 0)",
        ] {
            sqlx::query(sql).execute(pool).await.unwrap();
        }
    }

    #[tokio::test]
    async fn downgrade_strips_newer_schema_and_upgrade_restores_it() {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: this is the only test that touches the environment.
        unsafe { std::env::set_var("THESEUS_DB_BACKUP_DIR", dir.path()) };

        let options = SqliteConnectOptions::new()
            .filename(dir.path().join("app.db"))
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::query("CREATE TABLE app_metadata (key TEXT PRIMARY KEY, value TEXT, updated_at INTEGER)")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("INSERT OR REPLACE INTO app_metadata (key, value, updated_at) VALUES ('app_version', '9.9.9', 0)")
            .execute(&pool)
            .await
            .unwrap();

        pretend_newer_version(&pool).await;
        sqlx::query("UPDATE settings SET future_col = 'kept'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO future_table (id, v) VALUES (1, 'row')")
            .execute(&pool)
            .await
            .unwrap();

        // Downgrade: newer schema is gone and the old migrator accepts the database.
        let previous = prepare(&pool).await.unwrap();
        assert_eq!(previous.as_deref(), Some("9.9.9"));
        assert!(!has_table(&pool, "future_table").await.unwrap());
        assert!(
            sqlx::query("SELECT future_col FROM settings")
                .fetch_all(&pool)
                .await
                .is_err()
        );
        sqlx::migrate!().run(&pool).await.unwrap();
        assert!(prepare(&pool).await.unwrap().is_some());

        // Upgrade: the newer migrations run again, then the removed data returns.
        pretend_newer_version(&pool).await;
        restore_from(&pool, "9.9.9", APP_VERSION).await.unwrap();
        assert_eq!(
            value(&pool, "SELECT future_col FROM settings")
                .await
                .as_deref(),
            Some("kept")
        );
        assert_eq!(
            value(&pool, "SELECT v FROM future_table WHERE id = 1")
                .await
                .as_deref(),
            Some("row")
        );
    }
}
