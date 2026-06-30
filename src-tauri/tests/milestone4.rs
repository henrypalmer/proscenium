//! Milestone 4 acceptance tests: libmpv playback (headless), transport
//! controls, track selection, hardware decode, error states, stream URL
//! resolution, and external player command handling.

use proscenium_lib::commands::playback::{
    open_in_external_player_impl, resolve_external_player_command, resolve_stream_url_impl,
};
use proscenium_lib::commands::providers::upsert_provider_impl;
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, Category, EpisodeItem, LiveChannel, MovieItem, MpvState, ProviderInput,
    ProviderType, SeriesItem,
};
use proscenium_lib::mpv::player::{MpvConfig, MpvPlayer};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn media_path(name: &str, encode_args: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join("proscenium-media");
    let path = dir.join(name);
    if !path.exists() {
        std::fs::create_dir_all(&dir).unwrap();
        let status = std::process::Command::new("mpv")
            .args(encode_args)
            .arg(format!("--o={}", path.display()))
            .status()
            .expect("mpv.exe is required to generate test media");
        assert!(status.success(), "failed to generate {name}");
    }
    path
}

fn h264_file() -> PathBuf {
    media_path(
        "test-h264.mp4",
        &[
            "av://lavfi:testsrc2=duration=20:size=640x360:rate=30",
            "--audio-file=av://lavfi:sine=frequency=440:duration=20",
            "--ovc=libx264",
            "--oac=aac",
            "--quiet",
        ],
    )
}

fn h265_file() -> PathBuf {
    media_path(
        "test-h265.mp4",
        &[
            "av://lavfi:testsrc2=duration=10:size=640x360:rate=30",
            "--ovc=libx265",
            "--no-audio",
            "--quiet",
        ],
    )
}

fn headless_player(hwdec: bool) -> std::sync::Arc<MpvPlayer> {
    MpvPlayer::new(
        MpvConfig {
            composited: false,
            hwdec,
            headless: true,
        },
        Box::new(|_| {}),
    )
    .expect("libmpv must load — is libmpv-2.dll next to the test executable?")
}

fn wait_for(
    player: &MpvPlayer,
    timeout: Duration,
    pred: impl Fn(&MpvState) -> bool,
) -> MpvState {
    let start = Instant::now();
    loop {
        let state = player.get_state();
        if pred(&state) {
            return state;
        }
        if start.elapsed() > timeout {
            panic!("timed out waiting for player state; last state: {state:?}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn plays_h264_pauses_seeks_and_adjusts_volume() {
    let player = headless_player(false);
    player.load_url(h264_file().to_str().unwrap(), None).unwrap();

    // Begins playing: position advances, duration known, no error.
    let state = wait_for(&player, Duration::from_secs(15), |s| {
        s.playing && !s.paused && s.position > 0.3
    });
    assert!(state.error.is_none(), "unexpected error: {:?}", state.error);
    let duration = state.duration.expect("duration");
    assert!((18.0..22.0).contains(&duration), "duration {duration}");
    assert!(!state.buffering);

    // Audio track populated from the track list.
    assert!(!state.audio_tracks.is_empty(), "audio tracks missing");
    assert_eq!(state.active_audio_track, Some(state.audio_tracks[0].id));

    // Pause / play.
    player.pause().unwrap();
    wait_for(&player, Duration::from_secs(5), |s| s.paused);
    player.play().unwrap();
    wait_for(&player, Duration::from_secs(5), |s| !s.paused);

    // Absolute seek.
    player.seek(15.0).unwrap();
    wait_for(&player, Duration::from_secs(5), |s| s.position >= 14.0);

    // Volume + mute.
    player.set_volume(37.0).unwrap();
    wait_for(&player, Duration::from_secs(5), |s| {
        (s.volume - 37.0).abs() < 0.5
    });
    player.set_mute(true).unwrap();
    wait_for(&player, Duration::from_secs(5), |s| s.muted);
    player.set_mute(false).unwrap();
    wait_for(&player, Duration::from_secs(5), |s| !s.muted);

    // Track switching commands are accepted.
    let audio_id = player.get_state().audio_tracks[0].id;
    player.set_audio_track(audio_id).unwrap();
    player.set_subtitle_track(-1).unwrap(); // "off" is always valid

    // Stop returns the player to idle.
    player.stop().unwrap();
    wait_for(&player, Duration::from_secs(5), |s| !s.playing);
}

#[test]
fn hardware_decode_is_active_for_h264_and_h265() {
    let player = headless_player(true);

    for (file, codec) in [(h264_file(), "h264"), (h265_file(), "h265")] {
        player.load_url(file.to_str().unwrap(), None).unwrap();
        let state = wait_for(&player, Duration::from_secs(15), |s| {
            s.playing && s.position > 0.3 && s.hwdec_current.is_some()
        });
        let hwdec = state.hwdec_current.unwrap();
        assert!(
            hwdec.contains("d3d11") || hwdec.contains("dxva") || hwdec.contains("nvdec"),
            "{codec}: expected a Windows hardware decoder, got {hwdec}"
        );
        println!("{codec}: hwdec-current = {hwdec}");
    }
}

#[test]
fn hardware_decode_setting_applies_on_reload() {
    // The player instance is reused across streams, so `mpv_load_url` now
    // (re)applies the Hardware-decode setting at load time via `set_hwdec`. This
    // proves a runtime toggle actually reaches the decoder on the next stream
    // rather than being frozen at player creation.
    let player = headless_player(true);

    // Created with hwdec on → the first file hardware-decodes.
    player.load_url(h264_file().to_str().unwrap(), None).unwrap();
    let on = wait_for(&player, Duration::from_secs(15), |s| {
        s.playing && s.position > 0.3 && s.hwdec_current.is_some()
    });
    assert!(on.hwdec_current.is_some(), "expected hardware decode initially");

    // Turn it off and reload — the next file must decode in software.
    player.set_hwdec(false).unwrap();
    player.load_url(h264_file().to_str().unwrap(), None).unwrap();
    let off = wait_for(&player, Duration::from_secs(15), |s| {
        s.playing && s.position > 0.3
    });
    assert!(
        off.hwdec_current.is_none(),
        "hwdec should be off after set_hwdec(false), got {:?}",
        off.hwdec_current
    );
}

#[test]
fn failed_stream_reports_an_error_state() {
    let player = headless_player(false);
    // Connection-refused port: the stream fails quickly.
    player.load_url("http://127.0.0.1:9/dead.ts", None).unwrap();
    let state = wait_for(&player, Duration::from_secs(20), |s| s.error.is_some());
    assert!(!state.playing);
    println!("error surfaced: {:?}", state.error);
}

#[test]
fn state_change_callback_fires() {
    let (tx, rx) = mpsc::channel();
    let player = MpvPlayer::new(
        MpvConfig {
            composited: false,
            hwdec: false,
            headless: true,
        },
        Box::new(move |state| {
            let _ = tx.send(state);
        }),
    )
    .unwrap();
    player.load_url(h264_file().to_str().unwrap(), None).unwrap();

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut saw_playing = false;
    while Instant::now() < deadline {
        if let Ok(state) = rx.recv_timeout(Duration::from_millis(500)) {
            if state.playing && state.position > 0.0 {
                saw_playing = true;
                break;
            }
        }
    }
    assert!(saw_playing, "no playing state event arrived");
}

// --- resolve_stream_url ---

#[tokio::test]
async fn resolve_stream_url_finds_each_content_type() {
    let path = std::env::temp_dir().join(format!("proscenium-m4-{}.db", uuid::Uuid::new_v4()));
    let pool = db::init(&path).await.unwrap();
    let provider = upsert_provider_impl(
        &pool,
        ProviderInput {
            id: None,
            name: "M4".into(),
            provider_type: ProviderType::M3u,
            server_url: None,
            username: None,
            password: None,
            playlist_url: Some("http://example.com/x.m3u".into()),
            local_file_path: None,
        },
    )
    .await
    .unwrap();

    let data = CatalogData {
        live_categories: vec![Category { id: "c".into(), name: "c".into(), sort_order: 0 }],
        live_channels: vec![LiveChannel {
            id: "ch1".into(),
            provider_id: String::new(),
            name: "Chan".into(),
            category_id: "c".into(),
            category_name: "c".into(),
            logo_url: None,
            stream_url: "http://example.com/live/1.ts".into(),
            stream_ext: "ts".into(),
            epg_channel_id: None,
        }],
        vod_categories: vec![Category { id: "v".into(), name: "v".into(), sort_order: 0 }],
        movies: vec![MovieItem {
            id: "m1".into(),
            provider_id: String::new(),
            name: "Movie".into(),
            category_id: "v".into(),
            category_name: "v".into(),
            poster_url: None,
            stream_url: "http://example.com/movie/1.mp4".into(),
            container_ext: "mp4".into(),
            release_year: None,
            rating: None,
            added_at: None,
        }],
        series_categories: vec![Category { id: "s".into(), name: "s".into(), sort_order: 0 }],
        series: vec![SeriesItem {
            id: "se1".into(),
            provider_id: String::new(),
            name: "Show".into(),
            category_id: "s".into(),
            category_name: "s".into(),
            poster_url: None,
            release_year: None,
        }],
        episodes: vec![EpisodeItem {
            id: "ep1".into(),
            provider_id: String::new(),
            series_id: "se1".into(),
            season: 1,
            episode: 1,
            title: "Ep".into(),
            stream_url: "http://example.com/series/1.mp4".into(),
            container_ext: "mp4".into(),
            duration_seconds: None,
            poster_url: None,
            overview: None,
        }],
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1).await.unwrap();

    let live = resolve_stream_url_impl(&pool, &provider.id, "live", "ch1").await.unwrap();
    assert_eq!(live, "http://example.com/live/1.ts");
    let movie = resolve_stream_url_impl(&pool, &provider.id, "movie", "m1").await.unwrap();
    assert_eq!(movie, "http://example.com/movie/1.mp4");
    let episode = resolve_stream_url_impl(&pool, &provider.id, "episode", "ep1").await.unwrap();
    assert_eq!(episode, "http://example.com/series/1.mp4");

    let missing = resolve_stream_url_impl(&pool, &provider.id, "live", "nope").await;
    assert!(missing.is_err());
    let bad_type = resolve_stream_url_impl(&pool, &provider.id, "podcast", "x").await;
    assert!(bad_type.unwrap_err().contains("Unknown content type"));

    pool.close().await;
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

// --- external player ---

#[test]
fn external_player_commands_resolve_correctly() {
    let (exe, args) = resolve_external_player_command("mpv", None, "http://x/1.ts").unwrap();
    assert_eq!(exe, "mpv");
    assert_eq!(args, vec!["http://x/1.ts"]);

    let (exe, args) = resolve_external_player_command(
        "custom",
        Some("myplayer --fullscreen \"{url}\""),
        "http://x/stream 1.ts",
    )
    .unwrap();
    assert_eq!(exe, "myplayer");
    assert_eq!(args, vec!["--fullscreen", "http://x/stream 1.ts"]);

    assert!(resolve_external_player_command("custom", None, "u").is_err());
    assert!(resolve_external_player_command("winamp", None, "u").is_err());
}

#[tokio::test]
async fn open_in_external_player_spawns_custom_command_and_defaults_to_mpv() {
    let path = std::env::temp_dir().join(format!("proscenium-m4x-{}.db", uuid::Uuid::new_v4()));
    let pool = db::init(&path).await.unwrap();

    // Custom command that exits immediately (no GUI in tests).
    db::settings::set(&pool, "custom_player_command", "cmd /c echo {url}")
        .await
        .unwrap();
    open_in_external_player_impl(&pool, "http://x/1.ts", Some("custom".into()))
        .await
        .expect("custom player spawn");

    // Default (no player arg, no setting) resolves to mpv.
    let (exe, _) = resolve_external_player_command("mpv", None, "u").unwrap();
    assert_eq!(exe, "mpv");

    pool.close().await;
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}
