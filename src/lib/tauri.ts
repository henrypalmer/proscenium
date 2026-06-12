import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import { mockInvoke } from "./devMock";
import type {
  CatalogSummary,
  Category,
  ConnectionTestResult,
  EpisodesBySeason,
  LiveChannel,
  Movie,
  MovieDetail,
  MpvState,
  PaginatedResult,
  PlayableContentType,
  Provider,
  ProviderInput,
  Series,
  SeriesDetail,
} from "../types";

/** True when running inside the Tauri shell (vs. a plain browser). */
export const inTauri = isTauri();

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
  page: number,
  pageSize: number,
): Promise<PaginatedResult<LiveChannel>> {
  return invoke("get_live_channels", { providerId, categoryId, page, pageSize });
}

export function getVodCategories(providerId: string): Promise<Category[]> {
  return invoke("get_vod_categories", { providerId });
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

export const mpv = {
  loadUrl: (url: string): Promise<void> => invoke("mpv_load_url", { url }),
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
