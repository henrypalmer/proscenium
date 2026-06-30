//! Stremio addon storage (Milestone 41). Durable Tier-1 metadata for installed
//! stream addons. The token-bearing manifest URL is NOT here — it lives in the
//! OS keychain (`keychain::*_addon_secret`, account `addon:{id}`); this table
//! holds only the addon's non-secret declared metadata + a keychain reference.

use crate::models::StremioAddon;
use sqlx::{Row, SqlitePool};

fn json_array(s: String) -> Vec<String> {
    serde_json::from_str(&s).unwrap_or_default()
}

fn row_to_addon(r: &sqlx::sqlite::SqliteRow) -> StremioAddon {
    StremioAddon {
        id: r.get("id"),
        name: r.get("name"),
        types: json_array(r.get("types")),
        resources: json_array(r.get("resources")),
        id_prefixes: json_array(r.get("id_prefixes")),
        position: r.get("position"),
        created_at: r.get("created_at"),
    }
}

pub async fn insert(
    pool: &SqlitePool,
    addon: &StremioAddon,
    manifest_ref: &str,
) -> Result<(), sqlx::Error> {
    let types = serde_json::to_string(&addon.types).unwrap_or_else(|_| "[]".into());
    let resources = serde_json::to_string(&addon.resources).unwrap_or_else(|_| "[]".into());
    let id_prefixes = serde_json::to_string(&addon.id_prefixes).unwrap_or_else(|_| "[]".into());
    sqlx::query(
        "INSERT INTO stremio_addons
           (id, name, manifest_ref, types, resources, id_prefixes, position, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name, manifest_ref = excluded.manifest_ref,
           types = excluded.types, resources = excluded.resources,
           id_prefixes = excluded.id_prefixes, position = excluded.position",
    )
    .bind(&addon.id)
    .bind(&addon.name)
    .bind(manifest_ref)
    .bind(types)
    .bind(resources)
    .bind(id_prefixes)
    .bind(addon.position)
    .bind(addon.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<StremioAddon>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM stremio_addons ORDER BY position, created_at")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(row_to_addon).collect())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("DELETE FROM stremio_addons WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

/// The next append position (max + 1, or 0 when empty).
pub async fn next_position(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let row = sqlx::query("SELECT COALESCE(MAX(position), -1) + 1 AS p FROM stremio_addons")
        .fetch_one(pool)
        .await?;
    Ok(row.get("p"))
}
