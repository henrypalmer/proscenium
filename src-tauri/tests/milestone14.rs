//! Milestone 14 acceptance tests: custom lists / playlists (spec §5.11).
//! Create + mixed membership (movie/series/live), dedup, item resolution with
//! orphan filtering, cover/count summaries, "lists for item", removal, rename,
//! provider scoping and cascade deletion.

use proscenium_lib::commands::lists::{
    add_to_list_impl, create_list_impl, delete_list_impl, get_list_items_impl, get_lists_impl,
    get_lists_for_item_impl, remove_from_list_impl, rename_list_impl,
};
use proscenium_lib::commands::providers::{delete_provider_impl, upsert_provider_impl};
use proscenium_lib::db;
use proscenium_lib::models::{Provider, ProviderInput, ProviderType, UserListItem};
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m14-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

async fn make_provider(pool: &SqlitePool, name: &str) -> Provider {
    upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: name.into(),
            provider_type: ProviderType::M3u,
            server_url: None,
            username: None,
            password: None,
            playlist_url: Some("http://example.local/playlist.m3u".into()),
            local_file_path: None,
        },
    )
    .await
    .expect("provider")
}

async fn seed_movie(pool: &SqlitePool, provider_id: &str, id: &str, poster: Option<&str>) {
    sqlx::query(
        "INSERT INTO movies (id, provider_id, name, category_id, category_name, poster_url, stream_url, container_ext)
         VALUES (?, ?, ?, 'c', 'Cat', ?, 'http://s/m', 'mp4')",
    )
    .bind(id)
    .bind(provider_id)
    .bind(format!("Movie {id}"))
    .bind(poster)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_series(pool: &SqlitePool, provider_id: &str, id: &str) {
    sqlx::query(
        "INSERT INTO series (id, provider_id, name, category_id, category_name, poster_url)
         VALUES (?, ?, ?, 'c', 'Cat', 'http://p/s.jpg')",
    )
    .bind(id)
    .bind(provider_id)
    .bind(format!("Series {id}"))
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_channel(pool: &SqlitePool, provider_id: &str, id: &str) {
    sqlx::query(
        "INSERT INTO live_channels (id, provider_id, name, category_id, category_name, logo_url, stream_url, stream_ext)
         VALUES (?, ?, ?, 'c', 'Cat', 'http://l/c.png', 'http://s/c', 'ts')",
    )
    .bind(id)
    .bind(provider_id)
    .bind(format!("Channel {id}"))
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn lists_membership_resolution_and_scope() {
    let path = temp_path("crud");
    let pool = db::init(&path).await.expect("init");
    let p1 = make_provider(&pool, "P1").await;
    let p2 = make_provider(&pool, "P2").await;

    seed_movie(&pool, &p1.id, "m1", Some("http://p/m1.jpg")).await;
    seed_series(&pool, &p1.id, "s1").await;
    seed_channel(&pool, &p1.id, "l1").await;

    let list = create_list_impl(&pool, &p1.id, "Watch later").await.unwrap();

    // Mixed membership + an orphan (no catalog row) + a duplicate add (no-op).
    add_to_list_impl(&pool, &list.id, "movie", "m1").await.unwrap();
    add_to_list_impl(&pool, &list.id, "series", "s1").await.unwrap();
    add_to_list_impl(&pool, &list.id, "live", "l1").await.unwrap();
    add_to_list_impl(&pool, &list.id, "movie", "ghost").await.unwrap();
    add_to_list_impl(&pool, &list.id, "movie", "m1").await.unwrap(); // dedup

    // Episodes can't be added to lists.
    assert!(add_to_list_impl(&pool, &list.id, "episode", "e1").await.is_err());

    // Items resolve in insertion order; the orphan is filtered out.
    let items = get_list_items_impl(&pool, &list.id).await.unwrap();
    assert_eq!(items.len(), 3, "orphan + duplicate excluded");
    assert!(matches!(items[0], UserListItem::Movie { .. }));
    assert!(matches!(items[1], UserListItem::Series { .. }));
    assert!(matches!(items[2], UserListItem::Live { .. }));

    // Summary count excludes the orphan; covers carry posters.
    let summaries = get_lists_impl(&pool, &p1.id).await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].item_count, 3);
    assert_eq!(summaries[0].cover_posters.len(), 3);

    // "Lists for item" reflects membership.
    let for_m1 = get_lists_for_item_impl(&pool, &p1.id, "movie", "m1").await.unwrap();
    assert_eq!(for_m1, vec![list.id.clone()]);

    // Remove drops only the membership.
    remove_from_list_impl(&pool, &list.id, "movie", "m1").await.unwrap();
    assert_eq!(get_list_items_impl(&pool, &list.id).await.unwrap().len(), 2);
    assert!(get_lists_for_item_impl(&pool, &p1.id, "movie", "m1").await.unwrap().is_empty());

    // Rename is reflected in the summary.
    rename_list_impl(&pool, &list.id, "Renamed").await.unwrap();
    assert_eq!(get_lists_impl(&pool, &p1.id).await.unwrap()[0].list.name, "Renamed");

    // Provider-scoped: P2 sees no lists.
    assert!(get_lists_impl(&pool, &p2.id).await.unwrap().is_empty());

    // Deleting the provider cascade-removes its lists.
    delete_provider_impl(&pool, &p1.id).await.unwrap();
    assert!(get_lists_impl(&pool, &p1.id).await.unwrap().is_empty());

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn delete_list_removes_it_and_validates_name() {
    let path = temp_path("delete");
    let pool = db::init(&path).await.expect("init");
    let p = make_provider(&pool, "P").await;

    // Blank name is rejected.
    assert!(create_list_impl(&pool, &p.id, "   ").await.is_err());

    let list = create_list_impl(&pool, &p.id, "Temp").await.unwrap();
    assert_eq!(get_lists_impl(&pool, &p.id).await.unwrap().len(), 1);
    delete_list_impl(&pool, &list.id).await.unwrap();
    assert!(get_lists_impl(&pool, &p.id).await.unwrap().is_empty());

    pool.close().await;
    cleanup_db(&path);
}
