//! Milestone 14 + 39 acceptance tests: custom lists / playlists (spec §5.11).
//! Lists are **global** since Milestone 39 (not provider-scoped); each membership
//! row carries its item's `provider_id`, so a list can mix items from several
//! providers and the same content id from two providers can both be added.
//! Covers create + mixed cross-provider membership, dedup, orphan filtering,
//! cover/count summaries, provider-aware "lists for item", removal, rename, and
//! the fact that deleting a provider orphans (but does not delete) global lists.

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
async fn lists_are_global_and_mix_providers() {
    let path = temp_path("crud");
    let pool = db::init(&path).await.expect("init");
    let p1 = make_provider(&pool, "P1").await;
    let p2 = make_provider(&pool, "P2").await;

    seed_movie(&pool, &p1.id, "m1", Some("http://p/m1.jpg")).await;
    seed_series(&pool, &p1.id, "s1").await;
    seed_channel(&pool, &p1.id, "l1").await;
    // The same content id "m1" exists under a DIFFERENT provider too (Milestone 39).
    seed_movie(&pool, &p2.id, "m1", Some("http://p/m1b.jpg")).await;

    let list = create_list_impl(&pool, "Watch later").await.unwrap();

    // Mixed membership across providers + an orphan + a dedup no-op.
    add_to_list_impl(&pool, &list.id, &p1.id, "movie", "m1").await.unwrap();
    add_to_list_impl(&pool, &list.id, &p1.id, "series", "s1").await.unwrap();
    add_to_list_impl(&pool, &list.id, &p1.id, "live", "l1").await.unwrap();
    add_to_list_impl(&pool, &list.id, &p2.id, "movie", "m1").await.unwrap(); // same id, other provider
    add_to_list_impl(&pool, &list.id, &p1.id, "movie", "ghost").await.unwrap(); // orphan
    add_to_list_impl(&pool, &list.id, &p1.id, "movie", "m1").await.unwrap(); // dedup (no-op)

    // Episodes can't be added to lists.
    assert!(add_to_list_impl(&pool, &list.id, &p1.id, "episode", "e1").await.is_err());

    // Items resolve in insertion order; the orphan is filtered out; the same id
    // from two providers resolves to two distinct items.
    let items = get_list_items_impl(&pool, &list.id).await.unwrap();
    assert_eq!(items.len(), 4, "p1.m1, p1.s1, p1.l1, p2.m1 (orphan + dup excluded)");
    assert!(matches!(items[0], UserListItem::Movie { .. }));
    assert!(matches!(items[1], UserListItem::Series { .. }));
    assert!(matches!(items[2], UserListItem::Live { .. }));
    let movie_providers: Vec<&str> = items
        .iter()
        .filter_map(|it| match it {
            UserListItem::Movie { movie } => Some(movie.provider_id.as_str()),
            _ => None,
        })
        .collect();
    assert!(movie_providers.contains(&p1.id.as_str()));
    assert!(movie_providers.contains(&p2.id.as_str()), "cross-provider items coexist");

    // One global list; count excludes the orphan.
    let summaries = get_lists_impl(&pool).await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].item_count, 4);

    // "Lists for item" is provider-aware: both providers' m1 are in the list.
    let for_p1_m1 = get_lists_for_item_impl(&pool, &p1.id, "movie", "m1").await.unwrap();
    assert_eq!(for_p1_m1, vec![list.id.clone()]);
    let for_p2_m1 = get_lists_for_item_impl(&pool, &p2.id, "movie", "m1").await.unwrap();
    assert_eq!(for_p2_m1, vec![list.id.clone()]);

    // Remove drops only p1's m1 membership; p2's m1 stays.
    remove_from_list_impl(&pool, &list.id, &p1.id, "movie", "m1").await.unwrap();
    assert_eq!(get_list_items_impl(&pool, &list.id).await.unwrap().len(), 3);
    assert!(get_lists_for_item_impl(&pool, &p1.id, "movie", "m1").await.unwrap().is_empty());
    assert_eq!(
        get_lists_for_item_impl(&pool, &p2.id, "movie", "m1").await.unwrap(),
        vec![list.id.clone()]
    );

    // Rename is reflected in the (global) summary.
    rename_list_impl(&pool, &list.id, "Renamed").await.unwrap();
    assert_eq!(get_lists_impl(&pool).await.unwrap()[0].list.name, "Renamed");

    // Deleting a provider does NOT delete the global list; it orphans that
    // provider's items (filtered at read). p2's m1 remains resolvable.
    delete_provider_impl(&pool, &p1.id).await.unwrap();
    let after = get_lists_impl(&pool).await.unwrap();
    assert_eq!(after.len(), 1, "global list survives provider deletion");
    let items_after = get_list_items_impl(&pool, &list.id).await.unwrap();
    assert_eq!(items_after.len(), 1, "only p2's m1 still resolves");
    assert!(matches!(&items_after[0], UserListItem::Movie { movie } if movie.provider_id == p2.id));

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn delete_list_removes_it_and_validates_name() {
    let path = temp_path("delete");
    let pool = db::init(&path).await.expect("init");
    let _p = make_provider(&pool, "P").await;

    // Blank name is rejected.
    assert!(create_list_impl(&pool, "   ").await.is_err());

    let list = create_list_impl(&pool, "Temp").await.unwrap();
    assert_eq!(get_lists_impl(&pool).await.unwrap().len(), 1);
    delete_list_impl(&pool, &list.id).await.unwrap();
    assert!(get_lists_impl(&pool).await.unwrap().is_empty());

    pool.close().await;
    cleanup_db(&path);
}
