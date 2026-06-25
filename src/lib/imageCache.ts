import { convertFileSrc } from "@tauri-apps/api/core";
import { inTauri, resolveCachedImagePath, cacheImage } from "./tauri";

/**
 * Resolve a remote art URL to the `src` to actually load (spec §5.7,
 * Milestone 27). On a cache hit, returns the local file via the asset protocol
 * so no network request is made; on a miss, returns the remote URL and kicks
 * off a background download so the next view is served locally. Outside Tauri
 * (browser dev) there is no on-disk cache — the remote URL is used directly.
 */
export async function resolveArtSrc(url: string): Promise<string> {
  if (!inTauri) return url;
  try {
    const path = await resolveCachedImagePath(url);
    if (path) return convertFileSrc(path);
    // Cache miss: show the remote art now, populate the cache for next time.
    void cacheImage(url);
  } catch {
    // Fall through to the remote URL on any cache error.
  }
  return url;
}
