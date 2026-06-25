export type ProviderType = "xtream" | "m3u";

// Timestamps cross the Tauri IPC boundary as Unix seconds.
export interface Provider {
  id: string;
  name: string;
  type: ProviderType;
  serverUrl: string | null;
  username: string | null;
  playlistUrl: string | null;
  localFilePath: string | null;
  lastRefreshed: number | null;
  createdAt: number;
}

export interface ProviderInput {
  id?: string;
  name: string;
  type: ProviderType;
  serverUrl?: string;
  username?: string;
  password?: string;
  playlistUrl?: string;
  localFilePath?: string;
}

export interface XtreamAccountInfo {
  status: string | null;
  expDate: number | null;
  maxConnections: number | null;
  activeConnections: number | null;
}

export interface ConnectionTestResult {
  success: boolean;
  message: string;
  accountInfo: XtreamAccountInfo | null;
}

export interface Category {
  id: string;
  name: string;
  sortOrder: number;
}

export interface LiveChannel {
  id: string;
  name: string;
  categoryId: string;
  categoryName: string;
  logoUrl: string | null;
  streamExt: string;
  epgChannelId: string | null;
}

export interface Movie {
  id: string;
  name: string;
  categoryId: string;
  categoryName: string;
  posterUrl: string | null;
  containerExt: string;
  releaseYear: number | null;
  rating: string | null;
  addedAt: number | null;
}

export interface Series {
  id: string;
  name: string;
  categoryId: string;
  categoryName: string;
  posterUrl: string | null;
  releaseYear: number | null;
}

export interface Episode {
  id: string;
  seriesId: string;
  season: number;
  episode: number;
  title: string;
  containerExt: string;
  durationSeconds: number | null;
  posterUrl: string | null;
  /** Short episode synopsis (Xtream info.plot/overview; spec §5.4, M20). */
  overview: string | null;
}

/** Movie row plus on-demand metadata (Xtream vod_info; session-cached). */
export interface MovieDetail extends Movie {
  description: string | null;
  genre: string | null;
  durationSeconds: number | null;
  /** Wide hero backdrop (spec §5.4, M18); null falls back to the poster. */
  backdropUrl: string | null;
}

export interface SeriesDetail extends Series {
  description: string | null;
  genre: string | null;
  /** Wide hero backdrop (spec §5.4, M18); null falls back to the poster. */
  backdropUrl: string | null;
}

/** Keyed by season number (JSON object keys arrive as strings). */
export type EpisodesBySeason = Record<number, Episode[]>;

/** "More like this" related titles (spec §5.4, Milestone 28). Only the list
 * matching the requested content type is populated. */
export interface RelatedResults {
  movies: Movie[];
  series: Series[];
}

export interface PaginatedResult<T> {
  items: T[];
  total: number;
  page: number; // 1-based
  pageSize: number;
}

export interface CatalogSummary {
  liveChannels: number;
  movies: number;
  series: number;
}

export interface RefreshProgress {
  stage: string;
  progress: number; // 0..1
}

export interface RefreshComplete {
  success: boolean;
  error?: string;
}

export interface TrackInfo {
  id: number;
  title: string | null;
  lang: string | null;
  codec: string | null;
}

export interface MpvState {
  playing: boolean;
  paused: boolean;
  position: number; // seconds
  duration: number | null; // null for live streams
  volume: number; // 0-100
  muted: boolean;
  buffering: boolean;
  audioTracks: TrackInfo[];
  subtitleTracks: TrackInfo[];
  activeAudioTrack: number | null;
  activeSubtitleTrack: number | null;
  error: string | null;
  hwdecCurrent: string | null;
}

/** Persisted app settings (spec §15 settings keys). */
export interface AppSettings {
  activeProviderId: string | null;
  cacheTtlHours: number;
  defaultExternalPlayer: string;
  customPlayerCommand: string | null;
  uiDensity: string;
  uiTheme: string;
  hwDecodeEnabled: boolean;
  /** Image cache ceiling in MB (spec §5.7, Milestone 27). */
  imageCacheMaxMb: number;
}

export type ExternalPlayer = "mpv" | "vlc" | "custom";
export type UiDensity = "comfortable" | "compact";

/** Active-provider health for the startup warning banner (spec §12). */
export interface ProviderStatus {
  reachable: boolean;
  expired: boolean;
  message: string | null;
}

export type PlayableContentType = "live" | "movie" | "episode";

/** VOD content types that carry watch progress (spec §5.9; live is excluded). */
export type ProgressContentType = "movie" | "episode";

/** Saved watch progress for one VOD item (spec §5.9). */
export interface WatchProgress {
  positionSeconds: number;
  durationSeconds: number | null;
  completed: boolean;
  updatedAt: number; // Unix seconds
}

/** Content-type narrowing for the search command (spec §16). */
export type SearchContentType = "all" | "live" | "movies" | "series";

/** Local FTS search results, grouped by content type (spec §16). */
export interface SearchResults {
  liveChannels: LiveChannel[];
  movies: Movie[];
  series: Series[];
}

/** A custom user list / "playlist" (spec §5.11). */
export interface UserList {
  id: string;
  name: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

/** A list plus the data the Home "My Lists" cover card needs (spec §5.10):
 * count of resolvable items and up to four cover posters. */
export interface ListSummary extends UserList {
  itemCount: number;
  coverPosters: (string | null)[];
}

/** Content type that can be added to a custom list (spec §5.11). */
export type ListContentType = "live" | "movie" | "series";

/** One resolved list item (spec §5.11), discriminated by `kind`. */
export type UserListItem =
  | { kind: "movie"; movie: Movie }
  | { kind: "series"; series: Series }
  | { kind: "live"; channel: LiveChannel };

/** One Home "Keep Watching" item (spec §5.10): a movie or episode joined with
 * its catalog row plus the saved progress. Discriminated by `kind`. */
export type ContinueWatchingItem =
  | { kind: "movie"; movie: Movie; progress: WatchProgress }
  | {
      kind: "episode";
      episode: Episode;
      series: Series | null;
      progress: WatchProgress;
    };
