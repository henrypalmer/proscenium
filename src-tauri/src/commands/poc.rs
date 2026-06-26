//! Spike D POC: a localhost stream proxy so the WebView can play provider live
//! streams in `<video>` via MSE (mpegts.js / hls.js).
//!
//! Why it's needed: IPTV providers don't send CORS headers, so a direct
//! `fetch`/XHR from the WebView (mpegts.js's loader) is blocked. The proxy
//! resolves the real, keychain-composed URL **server-side** (so the password
//! never reaches the page) and pipes the bytes back from `127.0.0.1` with
//! permissive CORS. POC-only — delete with the spike.

use crate::commands::playback::resolve_stream_url_impl;
use sqlx::SqlitePool;
use tauri::State;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The bound proxy port, exposed to the frontend via `poc_proxy_base`.
pub struct PocProxy(pub u16);

/// Percent-decode a query value (POC-grade; handles `%XX` and `+`).
fn pct_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(b) => {
                    out.push(b);
                    i += 3;
                }
                Err(_) => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| pct_decode(v))
    })
}

async fn write_error(sock: &mut TcpStream, msg: &str) {
    let body = format!("proxy error: {msg}");
    let resp = format!(
        "HTTP/1.1 502 Bad Gateway\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = sock.write_all(resp.as_bytes()).await;
}

async fn handle(mut sock: TcpStream, pool: SqlitePool, client: reqwest::Client) {
    // Live TS is latency-sensitive; without TCP_NODELAY, Nagle coalesces the
    // small chunked writes and adds jitter the MSE buffer then has to absorb.
    let _ = sock.set_nodelay(true);
    let mut buf = vec![0u8; 8192];
    let n = match sock.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let line = req.lines().next().unwrap_or("");
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    if method == "OPTIONS" {
        let _ = sock
            .write_all(
                b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await;
        return;
    }

    let (path, query) = target.split_once('?').unwrap_or((target, ""));

    // Resolve the upstream URL (channel → keychain-composed real URL, or a
    // raw test URL passed through).
    let upstream = match path {
        "/live" => {
            let (Some(provider), Some(channel)) =
                (query_param(query, "provider"), query_param(query, "channel"))
            else {
                return write_error(&mut sock, "missing provider/channel").await;
            };
            match resolve_stream_url_impl(&pool, &provider, "live", &channel).await {
                Ok(url) => url,
                Err(e) => return write_error(&mut sock, &e).await,
            }
        }
        "/proxy" => match query_param(query, "url") {
            Some(url) => url,
            None => return write_error(&mut sock, "missing url").await,
        },
        _ => return write_error(&mut sock, "not found").await,
    };

    let resp = match client.get(&upstream).send().await {
        Ok(r) => r,
        Err(e) => return write_error(&mut sock, &format!("upstream: {e}")).await,
    };
    if !resp.status().is_success() {
        return write_error(&mut sock, &format!("upstream status {}", resp.status())).await;
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("video/mp2t")
        .to_string();

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
    );
    if sock.write_all(header.as_bytes()).await.is_err() {
        return;
    }

    // Pipe the (potentially infinite, live) body until the upstream ends or the
    // client disconnects. `chunk()` avoids needing a Stream combinator dep.
    let mut resp = resp;
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                if sock.write_all(&chunk).await.is_err() {
                    break; // client (the <video>) went away
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
}

/// Accept connections forever, one task per connection.
pub async fn accept_loop(listener: TcpListener, pool: SqlitePool, client: reqwest::Client) {
    loop {
        match listener.accept().await {
            Ok((sock, _)) => {
                tokio::spawn(handle(sock, pool.clone(), client.clone()));
            }
            Err(_) => break,
        }
    }
}

#[tauri::command]
pub async fn poc_proxy_base(state: State<'_, PocProxy>) -> Result<String, String> {
    Ok(format!("http://127.0.0.1:{}", state.0))
}
