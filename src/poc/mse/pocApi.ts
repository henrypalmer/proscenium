import { invoke as coreInvoke, isTauri } from "@tauri-apps/api/core";

/** POC (Spike D) IPC. Kept out of the production `lib/tauri.ts` so the spike is
 * self-contained and easy to delete. */

const inTauri = isTauri();

/** Base URL of the local stream proxy (e.g. `http://127.0.0.1:PORT`), or "" when
 * running outside Tauri (the proxy is a Rust task). */
export async function pocProxyBase(): Promise<string> {
  if (!inTauri) return "";
  try {
    return await coreInvoke<string>("poc_proxy_base");
  } catch {
    return "";
  }
}

/**
 * Proxy URL for a live channel. The backend resolves the real (keychain-composed)
 * stream server-side and pipes it back with permissive CORS, so the WebView's
 * `fetch` is same-origin (no provider CORS headers needed) and the provider
 * password never reaches the page.
 */
export function pocChannelUrl(
  base: string,
  providerId: string,
  channelId: string,
): string {
  return `${base}/live?provider=${encodeURIComponent(providerId)}&channel=${encodeURIComponent(channelId)}`;
}
