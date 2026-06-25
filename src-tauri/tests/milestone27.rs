//! Milestone 27 acceptance tests: the on-disk image cache pipeline (spec §5.7).
//! Covers the download-store-serve round trip, the LRU size cap (and that a
//! lookup bumps recency so a recently-viewed image survives eviction), the
//! missing-file self-heal, and the "Clear image cache" action. All offline
//! except the download test, which binds a throwaway local HTTP server.

use proscenium_lib::commands::images::{
    cache_image_impl, clear_image_cache_impl, enforce_size_cap, resolve_cached_image_impl,
    ImageCache,
};
use proscenium_lib::db;
use proscenium_lib::iptv::http_client;
use std::path::PathBuf;

fn temp_db(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m27-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("proscenium-m27-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

/// A throwaway server that answers every GET with a fixed image body.
async fn spawn_image_server(body: Vec<u8>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let body = body.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let _ = sock.read(&mut buf).await;
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(header.as_bytes()).await;
                let _ = sock.write_all(&body).await;
            });
        }
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn cache_image_downloads_stores_and_serves_from_disk() {
    let path = temp_db("download");
    let pool = db::init(&path).await.expect("init");
    let dir = temp_dir("download");
    let body = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let base = spawn_image_server(body.clone()).await;
    let url = format!("{base}/poster.png");
    let cache = ImageCache::new(dir.clone(), http_client().unwrap());
    let now = 1_700_000_000;

    // Miss → downloads, stores under the cache dir, returns the local path.
    let stored = cache_image_impl(&cache, &pool, &url, now, 500 * 1024 * 1024)
        .await
        .expect("download should store and return a path");
    assert!(std::path::Path::new(&stored).exists());
    assert!(stored.starts_with(dir.to_str().unwrap()));
    assert_eq!(std::fs::read(&stored).unwrap(), body);
    assert_eq!(
        db::image_cache::total_size(&pool).await.unwrap(),
        body.len() as i64
    );

    // Second view → cache hit served purely from SQLite + disk (no network).
    let hit = resolve_cached_image_impl(&pool, &url, now + 1)
        .await
        .expect("second view should hit the cache");
    assert_eq!(hit, stored);

    let _ = std::fs::remove_dir_all(&dir);
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn lru_size_cap_evicts_least_recently_accessed_first() {
    let path = temp_db("lru");
    let pool = db::init(&path).await.expect("init");
    let dir = temp_dir("lru");

    // Three 100-byte entries, accessed oldest→newest as a, b, c.
    let mut files = Vec::new();
    for (name, accessed) in [("a", 100i64), ("b", 200), ("c", 300)] {
        let file = dir.join(format!("{name}.bin"));
        std::fs::write(&file, vec![0u8; 100]).unwrap();
        db::image_cache::put(
            &pool,
            &format!("http://art.local/{name}.png"),
            file.to_str().unwrap(),
            100,
            accessed,
        )
        .await
        .unwrap();
        files.push((name, file));
    }
    assert_eq!(db::image_cache::total_size(&pool).await.unwrap(), 300);

    // Cap at 250 bytes → over by 50 → exactly the oldest ("a") is evicted.
    let evicted = enforce_size_cap(&pool, 250).await.expect("enforce");
    assert_eq!(evicted, 1);
    assert!(db::image_cache::total_size(&pool).await.unwrap() <= 250);

    assert!(!files[0].1.exists(), "least-recently-used file evicted");
    assert!(files[1].1.exists(), "newer files retained");
    assert!(files[2].1.exists(), "newest file retained");
    assert!(
        db::image_cache::lookup(&pool, "http://art.local/a.png", 999)
            .await
            .unwrap()
            .is_none(),
        "evicted row removed from the index"
    );

    let _ = std::fs::remove_dir_all(&dir);
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn lookup_bumps_recency_so_a_viewed_image_survives_eviction() {
    let path = temp_db("recency");
    let pool = db::init(&path).await.expect("init");
    let dir = temp_dir("recency");

    let file_a = dir.join("a.bin");
    let file_b = dir.join("b.bin");
    std::fs::write(&file_a, vec![0u8; 100]).unwrap();
    std::fs::write(&file_b, vec![0u8; 100]).unwrap();
    // a is older than b at insert time.
    db::image_cache::put(&pool, "http://art.local/a.png", file_a.to_str().unwrap(), 100, 100)
        .await
        .unwrap();
    db::image_cache::put(&pool, "http://art.local/b.png", file_b.to_str().unwrap(), 100, 200)
        .await
        .unwrap();

    // Viewing "a" again bumps it to the most-recent, so "b" is now the LRU.
    let _ = resolve_cached_image_impl(&pool, "http://art.local/a.png", 300).await;

    let evicted = enforce_size_cap(&pool, 150).await.expect("enforce");
    assert_eq!(evicted, 1);
    assert!(file_a.exists(), "recently-viewed image kept");
    assert!(!file_b.exists(), "stale image evicted instead");

    let _ = std::fs::remove_dir_all(&dir);
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn resolve_drops_row_when_cached_file_is_missing() {
    let path = temp_db("missing");
    let pool = db::init(&path).await.expect("init");

    // Row points at a file that isn't there (e.g. deleted out of band).
    db::image_cache::put(&pool, "http://art.local/gone.png", "/no/such/file.png", 50, 100)
        .await
        .unwrap();

    let resolved = resolve_cached_image_impl(&pool, "http://art.local/gone.png", 200).await;
    assert!(resolved.is_none(), "missing file is not served");
    assert!(
        db::image_cache::lookup(&pool, "http://art.local/gone.png", 999)
            .await
            .unwrap()
            .is_none(),
        "stale row is dropped so the next cache_image re-downloads"
    );

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn clear_image_cache_removes_files_and_rows() {
    let path = temp_db("clear");
    let pool = db::init(&path).await.expect("init");
    let dir = temp_dir("clear");

    let file = dir.join("poster.bin");
    std::fs::write(&file, vec![0u8; 100]).unwrap();
    db::image_cache::put(&pool, "http://art.local/p.png", file.to_str().unwrap(), 100, 100)
        .await
        .unwrap();

    clear_image_cache_impl(&pool, &dir).await.expect("clear");
    assert!(!file.exists(), "cached file removed");
    assert_eq!(db::image_cache::total_size(&pool).await.unwrap(), 0);

    let _ = std::fs::remove_dir_all(&dir);
    pool.close().await;
    cleanup_db(&path);
}
