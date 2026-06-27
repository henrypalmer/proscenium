import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import { mockInvoke } from "./devMock";
import type {
  AppSettings,
  CatalogSummary,
  Category,
  ConnectionTestResult,
  ContinueWatchingItem,
  EpisodesBySeason,
  ListContentType,
  ListSummary,
  LiveChannel,
  Movie,
  MovieDetail,
  MpvState,
  PaginatedResult,
  PlayableContentType,
  Provider,
  ProviderInput,
  ProviderStatus,
  RelatedResults,
  SearchContentType,
  SearchResults,
  Series,
  SeriesDetail,
  TileRect,
  ProgressContentType,
  UserList,
  UserListItem,
  WatchProgress,
} from "../types";

/** True when running inside the Tauri shell (vs. a plain browser). */
export const inTauri = isTauri();

/** True on the Tauri Windows build. Multi-view (Milestone 37) is Windows-only
 *  for now (the render compositor); its entry points hide elsewhere. */
export const isWindows =
  inTauri &&
  typeof navigator !== "undefined" &&
  /Windows NT/i.test(navigator.userAgent ?? "");

const invoke: typeof tauriInvoke = inTauri
  ? tauriInvoke
  : (mockInvoke as typeof tauriInvoke);

if (!inTauri) {
  console.info("[proscenium] running outside Tauri — using the dev mock backend");
}

export function upsertProvider(provider: ProviderInput): Promise<Provider> {
  return invoke("upsert_provider", { provider });
}

export function listProviders(): Promise<Provider[]> {
  return invoke("list_providers");
}

export function deleteProvider(providerId: string): Promise<void> {
  return invoke("delete_provider", { providerId });
}

export function testProviderConnection(
  provider: ProviderInput,
): Promise<ConnectionTestResult> {
  return invoke("test_provider_connection", { provider });
}

export function checkProviderStatus(providerId: string): Promise<ProviderStatus> {
  return invoke("check_provider_status", { providerId });
}

export function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export function setSetting(key: string, value: string): Promise<void> {
  return invoke("set_setting", { key, value });
}

// --- On-disk image cache (spec §5.7 / Milestone 27) ---

/** Local path of a cached art URL, or null on a miss (no download). */
export function resolveCachedImagePath(url: string): Promise<string | null> {
  return invoke("resolve_cached_image", { url });
}

/** Ensure an art URL is cached (downloads on a miss); resolves to the local
 * path or null. Used to populate the cache in the background. */
export function cacheImage(url: string): Promise<string | null> {
  return invoke("cache_image", { url });
}

/** Total bytes currently held by the on-disk image cache. */
export function imageCacheSize(): Promise<number> {
  return invoke("image_cache_size");
}

/** Delete every cached art file and empty the index. */
export function clearImageCache(): Promise<void> {
  return invoke("clear_image_cache");
}

export function getActiveProvider(): Promise<Provider | null> {
  return invoke("get_active_provider");
}

export function setActiveProvider(providerId: string): Promise<void> {
  return invoke("set_active_provider", { providerId });
}

export function refreshCatalog(providerId: string): Promise<void> {
  return invoke("refresh_catalog", { providerId });
}

export function getCatalogSummary(providerId: string): Promise<CatalogSummary> {
  return invoke("get_catalog_summary", { providerId });
}

export function getLiveCategories(providerId: string): Promise<Category[]> {
  return invoke("get_live_categories", { providerId });
}

export function getLiveChannels(
  providerId: string,
  categoryId: string | undefined,
  query: string | undefined,
  page: number,
  pageSize: number,
): Promise<PaginatedResult<LiveChannel>> {
  return invoke("get_live_channels", {
    providerId,
    categoryId,
    query,
    page,
    pageSize,
  });
}

export function getVodCategories(providerId: string): Promise<Category[]> {
  return invoke("get_vod_categories", { providerId });
}

// --- Recently-watched channels & custom category order (spec §13, M29) ---

export function recordRecentChannel(
  providerId: string,
  channelId: string,
): Promise<void> {
  return invoke("record_recent_channel", { providerId, channelId });
}

export function getRecentChannels(
  providerId: string,
  limit?: number,
): Promise<LiveChannel[]> {
  return invoke("get_recent_channels", { providerId, limit });
}

/** `section` is "live" | "movie" | "series". */
export function getCategoryOrder(
  providerId: string,
  section: string,
): Promise<string[]> {
  return invoke("get_category_order", { providerId, section });
}

export function setCategoryOrder(
  providerId: string,
  section: string,
  orderedIds: string[],
): Promise<void> {
  return invoke("set_category_order", { providerId, section, orderedIds });
}

export function getMovies(
  providerId: string,
  categoryId: string | undefined,
  page: number,
  pageSize: number,
): Promise<PaginatedResult<Movie>> {
  return invoke("get_movies", { providerId, categoryId, page, pageSize });
}

export function getSeriesCategories(providerId: string): Promise<Category[]> {
  return invoke("get_series_categories", { providerId });
}

export function getSeries(
  providerId: string,
  categoryId: string | undefined,
  page: number,
  pageSize: number,
): Promise<PaginatedResult<Series>> {
  return invoke("get_series", { providerId, categoryId, page, pageSize });
}

export function getEpisodes(
  providerId: string,
  seriesId: string,
): Promise<EpisodesBySeason> {
  return invoke("get_episodes", { providerId, seriesId });
}

export function getMovieDetail(
  providerId: string,
  movieId: string,
): Promise<MovieDetail> {
  return invoke("get_movie_detail", { providerId, movieId });
}

export function getSeriesDetail(
  providerId: string,
  seriesId: string,
): Promise<SeriesDetail> {
  return invoke("get_series_detail", { providerId, seriesId });
}

/** "More like this" related titles (spec §5.4, Milestone 28). Local-only —
 * same-category items of the same content type, excluding the title itself. */
export function getRelated(
  providerId: string,
  contentType: "movie" | "series",
  contentId: string,
  limit?: number,
): Promise<RelatedResults> {
  return invoke("get_related", { providerId, contentType, contentId, limit });
}

export function search(
  providerId: string,
  query: string,
  contentType?: SearchContentType,
  categoryId?: string,
  limit?: number,
): Promise<SearchResults> {
  return invoke("search", { providerId, query, contentType, categoryId, limit });
}

export function resolveStreamUrl(
  providerId: string,
  contentType: PlayableContentType,
  contentId: string,
): Promise<string> {
  return invoke("resolve_stream_url", { providerId, contentType, contentId });
}

export function openInExternalPlayer(
  streamUrl: string,
  player?: "mpv" | "vlc" | "custom",
): Promise<void> {
  return invoke("open_in_external_player", { streamUrl, player });
}

/**
 * Diagnose a failed stream load (spec §12, Milestone 22): probes the provider
 * and returns a user-facing reason distinguishing 4xx/5xx/network/timeout. The
 * backend logs a secret-redacted diagnostic line. Never rejects with the raw
 * mpv error — always resolves to a human-readable message.
 */
export function diagnosePlaybackFailure(
  providerId: string,
  contentType: PlayableContentType,
  contentId: string,
  mpvError: string | null,
): Promise<string> {
  return invoke("diagnose_playback_failure", {
    providerId,
    contentType,
    contentId,
    mpvError,
  });
}

/** Watch progress (spec §5.9). `contentType` is "movie" or "episode" only. */
export function getWatchProgress(
  providerId: string,
  contentType: ProgressContentType,
  contentId: string,
): Promise<WatchProgress | null> {
  return invoke("get_watch_progress", { providerId, contentType, contentId });
}

export function setWatchProgress(
  providerId: string,
  contentType: ProgressContentType,
  contentId: string,
  positionSeconds: number,
  durationSeconds: number | null,
): Promise<void> {
  return invoke("set_watch_progress", {
    providerId,
    contentType,
    contentId,
    positionSeconds,
    durationSeconds,
  });
}

/** Force an item to "watched" (Keep Watching → Mark as watched, spec §5.10). */
export function markWatched(
  providerId: string,
  contentType: ProgressContentType,
  contentId: string,
  durationSeconds: number | null,
): Promise<void> {
  return invoke("mark_watched", {
    providerId,
    contentType,
    contentId,
    durationSeconds,
  });
}

export function listWatchProgress(
  providerId: string,
  contentType: ProgressContentType,
): Promise<Record<string, WatchProgress>> {
  return invoke("list_watch_progress", { providerId, contentType });
}

export function clearWatchProgress(
  providerId: string,
  contentType: ProgressContentType,
  contentId: string,
): Promise<void> {
  return invoke("clear_watch_progress", { providerId, contentType, contentId });
}

/** In-progress movies/episodes for the Home "Keep Watching" row (spec §5.10). */
export function getContinueWatching(
  providerId: string,
  limit?: number,
): Promise<ContinueWatchingItem[]> {
  return invoke("get_continue_watching", { providerId, limit });
}

// --- Custom lists / playlists (spec §5.11 / §16) ---

export function createList(providerId: string, name: string): Promise<UserList> {
  return invoke("create_list", { providerId, name });
}

export function renameList(listId: string, name: string): Promise<void> {
  return invoke("rename_list", { listId, name });
}

export function deleteList(listId: string): Promise<void> {
  return invoke("delete_list", { listId });
}

export function reorderLists(
  providerId: string,
  orderedListIds: string[],
): Promise<void> {
  return invoke("reorder_lists", { providerId, orderedListIds });
}

export function getLists(providerId: string): Promise<ListSummary[]> {
  return invoke("get_lists", { providerId });
}

export function addToList(
  listId: string,
  contentType: ListContentType,
  contentId: string,
): Promise<void> {
  return invoke("add_to_list", { listId, contentType, contentId });
}

export function removeFromList(
  listId: string,
  contentType: ListContentType,
  contentId: string,
): Promise<void> {
  return invoke("remove_from_list", { listId, contentType, contentId });
}

export function reorderListItems(
  listId: string,
  orderedItemKeys: string[],
): Promise<void> {
  return invoke("reorder_list_items", { listId, orderedItemKeys });
}

export function getListItems(listId: string): Promise<UserListItem[]> {
  return invoke("get_list_items", { listId });
}

export function getListsForItem(
  providerId: string,
  contentType: ListContentType,
  contentId: string,
): Promise<string[]> {
  return invoke("get_lists_for_item", { providerId, contentType, contentId });
}

export const mpv = {
  loadUrl: (url: string, startSeconds?: number): Promise<void> =>
    invoke("mpv_load_url", { url, startSeconds }),
  play: (): Promise<void> => invoke("mpv_play"),
  pause: (): Promise<void> => invoke("mpv_pause"),
  stop: (): Promise<void> => invoke("mpv_stop"),
  seek: (seconds: number): Promise<void> => invoke("mpv_seek", { seconds }),
  setVolume: (volume: number): Promise<void> =>
    invoke("mpv_set_volume", { volume }),
  setMute: (muted: boolean): Promise<void> => invoke("mpv_set_mute", { muted }),
  setAudioTrack: (trackId: number): Promise<void> =>
    invoke("mpv_set_audio_track", { trackId }),
  setSubtitleTrack: (trackId: number): Promise<void> =>
    invoke("mpv_set_subtitle_track", { trackId }),
  getState: (): Promise<MpvState> => invoke("mpv_get_state"),
};

/** Multi-view tile control (Milestone 37). Tile 0 is the primary/single player;
 *  secondary tiles get ids 1.. Windows-only — the commands reject elsewhere. */
export const mv = {
  addTile: (
    providerId: string,
    contentId: string,
    x: number,
    y: number,
    w: number,
    h: number,
  ): Promise<number> =>
    invoke("mv_add_tile", { providerId, contentId, x, y, w, h }),
  removeTile: (tileId: number): Promise<void> =>
    invoke("mv_remove_tile", { tileId }),
  setRects: (rects: TileRect[]): Promise<void> =>
    invoke("mv_set_rects", { rects }),
  setActiveAudio: (tileId: number): Promise<void> =>
    invoke("mv_set_active_audio", { tileId }),
  setVolume: (tileId: number, volume: number): Promise<void> =>
    invoke("mv_set_volume", { tileId, volume }),
  close: (): Promise<void> => invoke("mv_close"),
};
