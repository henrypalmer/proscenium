/**
 * Browser-only mock backend. When the app runs outside the Tauri shell
 * (`npm run dev` in a plain browser) every invoke() is served from here so
 * the UI can be developed and exercised without the Rust backend.
 * Behavior mirrors the real commands: pagination, category filtering, and
 * case-insensitive alphabetical ordering.
 */

import type {
  AppSettings,
  CanonicalItem,
  CanonicalMeta,
  CanonicalVideo,
  CatalogSummary,
  Category,
  ConnectionTestResult,
  ContinueWatchingItem,
  Episode,
  EpisodesBySeason,
  ListSummary,
  LiveChannel,
  Movie,
  MovieDetail,
  MpvState,
  PaginatedResult,
  Provider,
  ProviderStatus,
  SearchResults,
  Series,
  SeriesDetail,
  StreamCandidate,
  UserList,
  UserListItem,
  WatchProgress,
} from "../types";

const LATENCY_MS = 350;
/** Fraction of runtime past which an item counts as fully watched (§5.9). */
const COMPLETION_THRESHOLD = 0.95;
const CHANNEL_COUNT = 12_000;
const MOVIE_COUNT = 12_000;
const SERIES_COUNT = 4_000;

const CATEGORY_NAMES = [
  "News", "Sports", "Movies HD", "Entertainment", "Kids", "Music",
  "Documentary", "Lifestyle", "Comedy", "Drama", "Science", "Travel",
  "Food", "History", "Nature", "Tech", "Local", "International",
  "Classics", "Premium", "Regional", "Weather", "Business", "Education",
  "Gaming", "Auto", "Outdoors", "Faith", "Shopping", "Late Night",
];

const FIRST = [
  "Alpha", "Bravo", "Comet", "Delta", "Echo", "Falcon", "Galaxy", "Horizon",
  "Iris", "Jupiter", "Kestrel", "Lumen", "Meteor", "Nova", "Orbit", "Pulse",
  "Quasar", "Ridge", "Summit", "Titan", "Umbra", "Vertex", "Zenith",
];
const SECOND = [
  "News", "Sports", "Cinema", "Kids", "Music", "Life", "World", "Prime",
  "Max", "One", "Plus",
];

const provider: Provider = {
  id: "mock-provider",
  name: "Mock Provider (browser dev)",
  type: "m3u",
  serverUrl: null,
  username: null,
  playlistUrl: "http://mock.local/playlist.m3u",
  localFilePath: null,
  lastRefreshed: Math.floor(Date.now() / 1000) - 3600,
  createdAt: Math.floor(Date.now() / 1000) - 86400,
};

// A second profile so the provider switcher (Milestone 36) is demoable in dev.
const provider2: Provider = {
  id: "mock-provider-2",
  name: "Second Provider (Xtream)",
  type: "xtream",
  serverUrl: "http://second.example:8080",
  username: "demo",
  playlistUrl: null,
  localFilePath: null,
  lastRefreshed: Math.floor(Date.now() / 1000) - 7200,
  createdAt: Math.floor(Date.now() / 1000) - 43200,
};

const mockProviders: Provider[] = [provider, provider2];
let mockActiveProviderId = provider.id;
// Enabled provider set (Milestone 39); starts with both so the merged catalog
// and badges are visible immediately in browser dev.
let mockEnabledProviderIds: string[] = [provider.id, provider2.id];

/** The enabled provider scope from a merged-read's args (Milestone 39). */
function scope(a: Record<string, unknown>): string[] {
  return (a.providerIds as string[] | undefined) ?? [];
}
function inScope(providerId: string, a: Record<string, unknown>): boolean {
  return scope(a).includes(providerId);
}

function svgLogo(seed: number): string {
  const hue = (seed * 47) % 360;
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80">` +
    `<rect width="80" height="80" fill="hsl(${hue},45%,35%)"/>` +
    `<text x="40" y="50" font-size="28" text-anchor="middle" fill="white" font-family="sans-serif">${seed % 100}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

let channelCache: LiveChannel[] | null = null;
function allChannels(): LiveChannel[] {
  if (!channelCache) {
    channelCache = Array.from({ length: CHANNEL_COUNT }, (_, i) => {
      const category = CATEGORY_NAMES[i % CATEGORY_NAMES.length];
      // One third real (data-URI) logos, one third broken URLs (placeholder
      // via onError), one third no logo at all.
      const logoUrl =
        i % 3 === 0 ? svgLogo(i) : i % 3 === 1 ? `http://invalid.local/logo-${i}.png` : null;
      // A few channels ship with blank names (as some real providers do), so the
      // M25 "Untitled channel" fallback is representable in browser dev.
      const blankName = i % 137 === 4;
      return {
        id: `live-${i}`,
        // A quarter belong to the second provider so the merged catalog + the
        // per-card provider badge are demoable in dev (Milestone 39).
        providerId: i % 4 === 0 ? provider2.id : provider.id,
        name: blankName
          ? ""
          : `${FIRST[i % FIRST.length]} ${SECOND[i % SECOND.length]} ${String(i % 997).padStart(3, "0")}`,
        categoryId: category,
        categoryName: category,
        logoUrl,
        streamExt: "ts",
        epgChannelId: null,
      };
    });
  }
  return channelCache;
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// Mutable settings store for browser dev (spec §15 defaults).
const mockSettings: AppSettings = {
  activeProviderId: provider.id,
  cacheTtlHours: 6,
  defaultExternalPlayer: "mpv",
  customPlayerCommand: null,
  uiDensity: "comfortable",
  uiTheme: "dark",
  hwDecodeEnabled: true,
  imageCacheMaxMb: 500,
};

// Browser dev has no on-disk cache; expose a demoable size so the Settings
// "Storage" controls are exercisable (Clear drops it to zero).
let mockImageCacheBytes = 42 * 1024 * 1024;

// Recently-watched channel ids (most-recent first) and custom category order
// (keyed by `${providerId}|${section}`) for Milestone 29.
const mockRecentChannelIds: string[] = [];
const mockCategoryOrder: Record<string, string[]> = {};

// --- Mock VOD catalog (Milestone 5) ---

const MOVIE_GENRES = [
  "Popular", "Action", "Comedy", "Drama", "Documentary", "Horror", "Sci-Fi",
  "Thriller", "Romance", "Animation", "Family", "Crime", "Adventure",
  "Fantasy", "Mystery", "War", "Western",
];
const SERIES_GENRES = [
  "Popular", "Crime", "Drama", "Comedy", "Sci-Fi", "Fantasy", "Reality",
  "Kids", "Documentary", "Anime", "Classic",
];
const TITLE_A = [
  "Midnight", "Crimson", "Silent", "Golden", "Broken", "Electric", "Hollow",
  "Savage", "Frozen", "Velvet", "Iron", "Neon", "Wandering", "Forgotten",
  "Burning", "Quiet", "Distant", "Shattered", "Lucky", "Final",
];
const TITLE_B = [
  "Horizon", "Empire", "Protocol", "River", "Garden", "Vendetta", "Echo",
  "Harvest", "Voyage", "Reckoning", "Country", "Signal", "Paradox", "Mirage",
  "Covenant", "Frontier", "Labyrinth", "Sonata", "Gambit", "Requiem",
];

/** 2:3 poster art data-URI; same null/broken/real mix as channel logos. */
function svgPoster(seed: number, title: string): string {
  const hue = (seed * 67) % 360;
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="200" height="300">` +
    `<rect width="200" height="300" fill="hsl(${hue},40%,28%)"/>` +
    `<rect x="12" y="12" width="176" height="276" fill="none" stroke="hsl(${hue},45%,45%)" stroke-width="3"/>` +
    `<text x="100" y="160" font-size="20" text-anchor="middle" fill="white" font-family="sans-serif">${encodeURIComponent(title).slice(0, 2)}${seed % 100}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

/** Wide 16:9 hero backdrop data-URI (spec §5.4, M18). */
function svgBackdrop(seed: number, title: string): string {
  const hue = (seed * 67) % 360;
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
    `<stop offset="0" stop-color="hsl(${hue},45%,32%)"/>` +
    `<stop offset="1" stop-color="hsl(${(hue + 40) % 360},50%,16%)"/></linearGradient></defs>` +
    `<rect width="1280" height="720" fill="url(#g)"/>` +
    `<text x="80" y="600" font-size="120" fill="hsla(${hue},60%,75%,0.25)" font-family="sans-serif">${encodeURIComponent(title).slice(0, 2)}${seed % 100}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

function posterFor(i: number, title: string): string | null {
  return i % 3 === 0
    ? svgPoster(i, title)
    : i % 3 === 1
      ? `http://invalid.local/poster-${i}.jpg`
      : null;
}

// --- Canonical catalog (Milestone 40 dev stand-in for Cinemeta) ---

const CANONICAL_GENRES_MOVIE = [
  "Action", "Adventure", "Animation", "Biography", "Comedy", "Crime",
  "Documentary", "Drama", "Family", "Fantasy", "History", "Horror", "Mystery",
  "Romance", "Sci-Fi", "Sport", "Thriller", "War", "Western",
];
const CANONICAL_GENRES_SERIES = [
  ...CANONICAL_GENRES_MOVIE, "Reality-TV", "Talk-Show", "Game-Show",
];

function canonicalName(seed: number): string {
  return `${TITLE_A[seed % TITLE_A.length]} ${TITLE_B[(seed * 7) % TITLE_B.length]}`;
}

function canonicalImdbId(kind: "movie" | "series", seed: number): string {
  return `tt${kind === "series" ? "9" : "1"}${String(seed).padStart(6, "0")}`;
}

/** A deterministic canonical page (~24 per page, 3 pages deep so "load more"
 * terminates). Mirrors the real Cinemeta catalog shape. */
function mockCanonicalCatalog(
  kind: "movie" | "series",
  genre: string | undefined,
  search: string | undefined,
  skip: number,
): CanonicalItem[] {
  const PAGE = 24;
  if (skip >= PAGE * 3) return [];
  const base =
    (genre ? genre.length * 31 + genre.charCodeAt(0) : 0) +
    (kind === "series" ? 5000 : 0);
  const items: CanonicalItem[] = [];
  for (let n = 0; n < PAGE; n++) {
    const seed = base + skip + n;
    const name = canonicalName(seed);
    if (search && !name.toLowerCase().includes(search.toLowerCase())) continue;
    items.push({
      imdbId: canonicalImdbId(kind, seed),
      kind,
      name,
      posterUrl: posterFor(seed, name),
      releaseYear: 1990 + (seed % 35),
    });
  }
  return items;
}

function mockCanonicalMeta(kind: "movie" | "series", imdbId: string): CanonicalMeta {
  const seed = Number(imdbId.replace(/\D/g, "")) || 1;
  const name = canonicalName(seed);
  const videos: CanonicalVideo[] = [];
  if (kind === "series") {
    for (let s = 1; s <= 2; s++) {
      for (let e = 1; e <= 6; e++) {
        videos.push({
          id: `${imdbId}:${s}:${e}`,
          season: s,
          episode: e,
          name: `Episode ${e}`,
          overview: "Mock episode synopsis from the dev backend.",
          thumbnail: svgBackdrop(seed + s * 7 + e, name),
          released: null,
        });
      }
    }
  }
  return {
    imdbId,
    kind,
    name,
    posterUrl: svgPoster(seed, name),
    backdropUrl: svgBackdrop(seed, name),
    description:
      "A canonical title resolved from the dev mock's Cinemeta stand-in. Source resolution across providers arrives in a later M40 slice.",
    releaseYear: 1990 + (seed % 35),
    releaseInfo: String(1990 + (seed % 35)),
    genres: [kind === "series" ? "Drama" : "Action", "Adventure"],
    cast: ["A. Mock", "B. Stand-in", "C. Placeholder"],
    director: ["D. Devmock"],
    runtime: kind === "movie" ? "118 min" : null,
    imdbRating: Math.round((6 + (seed % 40) / 10) * 10) / 10,
    tmdbId: 100000 + seed,
    videos,
  };
}

/** Fake source resolution: candidates point at real mock movies (so the play
 * path resolves), with ~1 in 5 titles returning none to exercise "no sources". */
function mockResolveSources(
  kind: "movie" | "series",
  imdbId: string,
  season?: number,
  episode?: number,
): StreamCandidate[] {
  const seed = Number(imdbId.replace(/\D/g, "")) || 0;
  if (kind === "series") {
    if (season == null || episode == null) return [];
    if ((seed + season * 10 + episode) % 5 === 0) return []; // some episodes: no source
    const epQualities = ["1080p", "720p"];
    return mockEnabledProviderIds.slice(0, 2).map((pid, i) => ({
      source: mockProviders.find((p) => p.id === pid)?.name ?? pid,
      providerId: pid,
      contentType: "episode" as const,
      contentId: `ep-${seed % 50}-${season}-${episode}`,
      url: null,
      quality: epQualities[i] ?? null,
      container: "mkv",
      confidence: 1 - i * 0.1,
    }));
  }
  if (seed % 5 === 0) return [];
  const movies = allMovies().filter((m) => mockEnabledProviderIds.includes(m.providerId));
  if (movies.length === 0) return [];
  const qualities = ["2160p", "1080p", "720p"];
  const seen = new Set<string>();
  const out: StreamCandidate[] = [];
  [seed, seed * 7 + 1, seed * 13 + 2].forEach((n, i) => {
    const m = movies[n % movies.length];
    const key = `${m.providerId}:${m.id}`;
    if (seen.has(key)) return;
    seen.add(key);
    out.push({
      source: mockProviders.find((p) => p.id === m.providerId)?.name ?? m.providerId,
      providerId: m.providerId,
      contentType: "movie",
      contentId: m.id,
      url: null,
      quality: qualities[i] ?? null,
      container: m.containerExt,
      confidence: 1 - i * 0.1,
    });
  });
  return out;
}

let movieCache: Movie[] | null = null;
function allMovies(): Movie[] {
  if (!movieCache) {
    movieCache = Array.from({ length: MOVIE_COUNT }, (_, i) => {
      const genre = MOVIE_GENRES[i % MOVIE_GENRES.length];
      const name = `${TITLE_A[i % TITLE_A.length]} ${TITLE_B[(i * 7) % TITLE_B.length]} ${String(i % 887).padStart(3, "0")}`;
      return {
        id: `movie-${i}`,
        providerId: i % 4 === 0 ? provider2.id : provider.id,
        name,
        categoryId: genre,
        categoryName: genre,
        posterUrl: posterFor(i, name),
        containerExt: "mp4",
        releaseYear: i % 7 === 0 ? null : 1965 + (i % 60),
        rating: i % 5 === 0 ? null : ((i % 70) / 10 + 2.9).toFixed(1),
        addedAt: null,
      };
    });
  }
  return movieCache;
}

let seriesCache: Series[] | null = null;
function allSeries(): Series[] {
  if (!seriesCache) {
    seriesCache = Array.from({ length: SERIES_COUNT }, (_, i) => {
      const genre = SERIES_GENRES[i % SERIES_GENRES.length];
      const name = `${TITLE_A[(i * 3) % TITLE_A.length]} ${TITLE_B[i % TITLE_B.length]} ${String(i % 397).padStart(3, "0")}`;
      return {
        id: `series-${i}`,
        providerId: i % 4 === 0 ? provider2.id : provider.id,
        name,
        categoryId: genre,
        categoryName: genre,
        posterUrl: posterFor(i, name),
        releaseYear: i % 6 === 0 ? null : 1990 + (i % 35),
      };
    });
  }
  return seriesCache;
}

function episodesFor(seriesId: string): EpisodesBySeason {
  const n = Number(seriesId.replace("series-", "")) || 0;
  const seasons = 1 + (n % 4);
  const grouped: EpisodesBySeason = {};
  for (let s = 1; s <= seasons; s++) {
    const count = 6 + ((n + s) % 7);
    grouped[s] = Array.from({ length: count }, (_, e): Episode => {
      const name = TITLE_B[(n + s + e) % TITLE_B.length];
      return {
        id: `ep-${n}-${s}-${e + 1}`,
        providerId: n % 4 === 0 ? provider2.id : provider.id,
        seriesId,
        season: s,
        episode: e + 1,
        // Provider-style redundant title (series/SxxEyy embedded) so the
        // frontend's cleanEpisodeTitle normalization is exercised in dev.
        title: `S${String(s).padStart(2, "0")}E${String(e + 1).padStart(2, "0")} — ${name}`,
        containerExt: "mp4",
        durationSeconds: 1260 + ((n + e) % 5) * 300,
        // Mix of real (SVG), broken, and missing art so the thumbnail's
        // loaded/error/placeholder states are all reachable in dev.
        posterUrl: posterFor(n * 13 + s * 7 + e, name),
        overview:
          (n + s + e) % 5 === 0
            ? null // some episodes have no synopsis
            : `${name} forces a reckoning: alliances shift, a secret surfaces, and no one walks away unchanged.`,
      };
    });
  }
  return grouped;
}

/** Shared pagination: case-insensitive alphabetical, optional genre filter
 * (mirrors the backend's ORDER BY name COLLATE NOCASE). */
function paginateByName<T extends { name: string; categoryId: string; providerId: string }>(
  list: T[],
  a: Args,
): PaginatedResult<T> {
  const categoryId = a.categoryId as string | undefined;
  // Optional in-section name filter (spec §5.3 — used by get_live_channels).
  const query = ((a.query as string | undefined) ?? "").trim().toLowerCase();
  const page = Math.max(1, (a.page as number) ?? 1);
  const pageSize = Math.min(500, Math.max(1, (a.pageSize as number) ?? 200));
  const filtered = list
    // Milestone 39: merged across the enabled provider set; category is by name.
    .filter((item) => inScope(item.providerId, a))
    .filter((item) => !categoryId || item.categoryId === categoryId)
    .filter((item) => !query || item.name.toLowerCase().includes(query))
    .sort((x, y) => x.name.toLowerCase().localeCompare(y.name.toLowerCase()));
  const start = (page - 1) * pageSize;
  return {
    items: filtered.slice(start, start + pageSize),
    total: filtered.length,
    page,
    pageSize,
  };
}

/** Mirrors the backend's FTS5 prefix search (Milestone 6): every
 * whitespace token must prefix-match a word in the name or category name. */
function mockSearch(a: Args): SearchResults {
  const tokens = ((a.query as string) ?? "")
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
  const contentType = (a.contentType as string | undefined) ?? "all";
  const categoryId = a.categoryId as string | undefined;
  const limit = Math.min(500, Math.max(1, (a.limit as number) ?? 20));

  function take<
    T extends { name: string; categoryId: string; categoryName: string; providerId: string },
  >(list: T[], wanted: string): T[] {
    if (tokens.length === 0 || (contentType !== "all" && contentType !== wanted)) {
      return [];
    }
    return list
      .filter((item) => {
        if (!inScope(item.providerId, a)) return false;
        if (categoryId && item.categoryId !== categoryId) return false;
        const words = `${item.name} ${item.categoryName}`.toLowerCase().split(/\s+/);
        return tokens.every((t) => words.some((w) => w.startsWith(t)));
      })
      .sort((x, y) => x.name.toLowerCase().localeCompare(y.name.toLowerCase()))
      .slice(0, limit);
  }

  return {
    liveChannels: take(allChannels(), "live"),
    movies: take(allMovies(), "movies"),
    series: take(allSeries(), "series"),
  };
}

// --- Mock watch progress store (Milestone 8) ---

const watchProgress = new Map<string, WatchProgress>();
const wpKey = (providerId: string, contentType: string, contentId: string) =>
  `${providerId}|${contentType}|${contentId}`;

// Pre-seed a little history so the Home "Keep Watching" row (Milestone 10) is
// populated in browser dev. One in-progress movie, one in-progress episode,
// and one completed movie (which must NOT appear in Keep Watching).
(() => {
  const nowS = Math.floor(Date.now() / 1000);
  watchProgress.set(wpKey(provider.id, "movie", "movie-3"), {
    positionSeconds: 1800,
    durationSeconds: 5400,
    completed: false,
    updatedAt: nowS - 120,
  });
  watchProgress.set(wpKey(provider.id, "episode", "ep-2-1-2"), {
    positionSeconds: 600,
    durationSeconds: 2160,
    completed: false,
    updatedAt: nowS - 40,
  });
  watchProgress.set(wpKey(provider.id, "movie", "movie-7"), {
    positionSeconds: 5300,
    durationSeconds: 5400,
    completed: true,
    updatedAt: nowS - 10,
  });
})();

/** Resolve a mock episode id (`ep-{series}-{season}-{ep}`) back to its episode
 * row and parent series — backs the Keep Watching join (Milestone 10). */
function findEpisodeById(
  id: string,
): { episode: Episode; series: Series | null } | null {
  const m = /^ep-(\d+)-(\d+)-(\d+)$/.exec(id);
  if (!m) return null;
  const seriesId = `series-${m[1]}`;
  const episode = episodesFor(seriesId)[Number(m[2])]?.find((e) => e.id === id);
  if (!episode) return null;
  return { episode, series: allSeries().find((s) => s.id === seriesId) ?? null };
}

// --- Mock custom lists / playlists (Milestone 14) ---

interface MockList {
  id: string;
  name: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}
interface MockListItem {
  providerId: string;
  contentType: "live" | "movie" | "series";
  contentId: string;
  position: number;
}

const userLists = new Map<string, MockList>();
const userListItems = new Map<string, MockListItem[]>(); // listId -> items
let listSeq = 0;
const newListId = () => `list-${++listSeq}`;

/** Resolve a list item to its catalog card, or null if the content is gone. */
function resolveListItem(item: MockListItem): UserListItem | null {
  const sameProvider = (x: { providerId: string }) => x.providerId === item.providerId;
  if (item.contentType === "movie") {
    const movie = allMovies().find((m) => m.id === item.contentId && sameProvider(m));
    return movie ? { kind: "movie", movie } : null;
  }
  if (item.contentType === "series") {
    const series = allSeries().find((s) => s.id === item.contentId && sameProvider(s));
    return series ? { kind: "series", series } : null;
  }
  const channel = allChannels().find((c) => c.id === item.contentId && sameProvider(c));
  return channel ? { kind: "live", channel } : null;
}

function resolvedItemsFor(listId: string): UserListItem[] {
  const items = [...(userListItems.get(listId) ?? [])].sort(
    (a, b) => a.position - b.position,
  );
  return items
    .map(resolveListItem)
    .filter((x): x is UserListItem => x !== null);
}

function posterOf(item: UserListItem): string | null {
  if (item.kind === "movie") return item.movie.posterUrl;
  if (item.kind === "series") return item.series.posterUrl;
  return item.channel.logoUrl;
}

function listSummary(list: MockList): ListSummary {
  const resolved = resolvedItemsFor(list.id);
  return {
    id: list.id,
    name: list.name,
    sortOrder: list.sortOrder,
    createdAt: list.createdAt,
    updatedAt: list.updatedAt,
    itemCount: resolved.length,
    coverPosters: resolved.slice(0, 4).map(posterOf),
  };
}

// Pre-seed a couple of lists so the Home "My Lists" row (Milestone 15) shows in
// browser dev.
(() => {
  const nowS = Math.floor(Date.now() / 1000);
  const horror: MockList = {
    id: newListId(),
    name: "Horror movies to watch",
    sortOrder: 0,
    createdAt: nowS - 500,
    updatedAt: nowS - 100,
  };
  const binge: MockList = {
    id: newListId(),
    name: "Binge Worthy TV Shows",
    sortOrder: 1,
    createdAt: nowS - 400,
    updatedAt: nowS - 50,
  };
  userLists.set(horror.id, horror);
  userLists.set(binge.id, binge);
  const pid = provider.id;
  userListItems.set(horror.id, [
    { providerId: pid, contentType: "movie", contentId: "movie-1", position: 0 },
    { providerId: pid, contentType: "movie", contentId: "movie-5", position: 1 },
    { providerId: pid, contentType: "live", contentId: "live-2", position: 2 },
  ]);
  userListItems.set(binge.id, [
    { providerId: pid, contentType: "series", contentId: "series-1", position: 0 },
    { providerId: pid, contentType: "series", contentId: "series-3", position: 1 },
  ]);
})();

// --- Mock mpv state machine (drives the player UI in browser dev) ---

function defaultMpvState(): MpvState {
  return {
    playing: false,
    paused: false,
    position: 0,
    duration: null,
    volume: 100,
    muted: false,
    buffering: false,
    audioTracks: [],
    subtitleTracks: [],
    activeAudioTrack: null,
    activeSubtitleTrack: null,
    error: null,
    hwdecCurrent: null,
  };
}

const mockMpv = {
  state: defaultMpvState(),
  ticker: null as number | null,
  lastContentType: "live" as string,

  load(url: string, startSeconds?: number) {
    this.stopTicker();
    const resumeAt = startSeconds && startSeconds > 0 ? startSeconds : 0;
    this.state = {
      ...defaultMpvState(),
      volume: this.state.volume,
      muted: this.state.muted,
      buffering: true,
    };
    // URLs containing "fail" simulate a dead stream; "slow" buffers forever
    // (exercises the 10s notice / 30s error thresholds).
    if (url.includes("fail")) {
      window.setTimeout(() => {
        this.state = {
          ...this.state,
          buffering: false,
          error: "loading failed (simulated)",
        };
      }, 1000);
      return;
    }
    if (url.includes("slow")) return; // buffering never resolves
    window.setTimeout(() => {
      this.state = {
        ...this.state,
        playing: true,
        paused: false,
        buffering: false,
        position: resumeAt,
        duration: this.lastContentType === "live" ? null : 1320,
        audioTracks: [
          { id: 1, title: "Stereo", lang: "eng", codec: "aac" },
          { id: 2, title: "Surround 5.1", lang: "eng", codec: "ac3" },
        ],
        subtitleTracks: [{ id: 1, title: "English", lang: "eng", codec: "subrip" }],
        activeAudioTrack: 1,
        activeSubtitleTrack: null,
        hwdecCurrent: "d3d11va (mock)",
      };
      this.ticker = window.setInterval(() => {
        if (this.state.playing && !this.state.paused) {
          this.state = { ...this.state, position: this.state.position + 0.4 };
        }
      }, 400);
    }, 1200);
  },

  stopTicker() {
    if (this.ticker !== null) {
      window.clearInterval(this.ticker);
      this.ticker = null;
    }
  },
};

type Args = Record<string, unknown>;

// Multi-view (Milestone 37) dev state: the browser mock has no real second
// stream, so it just hands out tile ids so the grid UI can be exercised.
let mockMvNextId = 0;

export async function mockInvoke<T>(cmd: string, args?: unknown): Promise<T> {
  // Search is a local FTS query in the real backend (~1ms); the simulated
  // network latency would misrepresent it (spec §10: results < 300ms).
  await sleep(cmd === "search" ? 30 : LATENCY_MS);
  const a = (args ?? {}) as Args;
  switch (cmd) {
    case "list_providers":
      return mockProviders as T;
    case "get_active_provider":
      return (mockProviders.find((p) => p.id === mockActiveProviderId) ?? null) as T;
    case "set_active_provider":
      mockActiveProviderId = a.providerId as string;
      return undefined as T;
    case "get_enabled_providers":
      return mockProviders.filter((p) => mockEnabledProviderIds.includes(p.id)) as T;
    case "set_enabled_providers":
      mockEnabledProviderIds = (a.providerIds as string[]) ?? [];
      if (mockEnabledProviderIds.length > 0) mockActiveProviderId = mockEnabledProviderIds[0];
      return undefined as T;
    case "refresh_catalog": {
      // Stamp a fresh "Last refreshed" time so the timestamp update is demoable.
      const active = mockProviders.find((p) => p.id === mockActiveProviderId);
      if (active) active.lastRefreshed = Math.floor(Date.now() / 1000);
      return undefined as T;
    }
    case "get_catalog_summary": {
      const ids = scope(a);
      return {
        liveChannels: allChannels().filter((c) => ids.includes(c.providerId)).length,
        movies: allMovies().filter((m) => ids.includes(m.providerId)).length,
        series: allSeries().filter((s) => ids.includes(s.providerId)).length,
      } satisfies CatalogSummary as T;
    }
    case "get_live_categories":
      return CATEGORY_NAMES.map((name, i) => ({
        id: name,
        name,
        sortOrder: i,
      })) satisfies Category[] as T;
    case "get_live_channels":
      return paginateByName(allChannels(), a) satisfies PaginatedResult<LiveChannel> as T;
    case "record_recent_channel": {
      const id = a.channelId as string;
      const idx = mockRecentChannelIds.indexOf(id);
      if (idx >= 0) mockRecentChannelIds.splice(idx, 1);
      mockRecentChannelIds.unshift(id);
      return undefined as T;
    }
    case "get_recent_channels": {
      const limit = (a.limit as number) ?? 15;
      const byId = new Map(allChannels().map((c) => [c.id, c]));
      const items = mockRecentChannelIds
        .map((id) => byId.get(id))
        .filter((c): c is LiveChannel => !!c && inScope(c.providerId, a))
        .slice(0, limit);
      return items as T;
    }
    case "get_category_order":
      return (mockCategoryOrder[`${a.providerId}|${a.section}`] ?? []) as T;
    case "set_category_order":
      mockCategoryOrder[`${a.providerId}|${a.section}`] =
        (a.orderedIds as string[]) ?? [];
      return undefined as T;
    case "get_vod_categories":
      return MOVIE_GENRES.map((name, i) => ({
        id: name,
        name,
        sortOrder: i,
      })) satisfies Category[] as T;
    case "get_movies":
      return paginateByName(allMovies(), a) satisfies PaginatedResult<Movie> as T;
    case "get_series_categories":
      return SERIES_GENRES.map((name, i) => ({
        id: name,
        name,
        sortOrder: i,
      })) satisfies Category[] as T;
    case "get_series":
      return paginateByName(allSeries(), a) satisfies PaginatedResult<Series> as T;
    case "get_canonical_genres":
      return (a.kind === "series"
        ? CANONICAL_GENRES_SERIES
        : CANONICAL_GENRES_MOVIE) as T;
    case "get_canonical_catalog":
      return mockCanonicalCatalog(
        a.kind as "movie" | "series",
        a.genre as string | undefined,
        a.search as string | undefined,
        (a.skip as number) ?? 0,
      ) satisfies CanonicalItem[] as T;
    case "get_canonical_meta":
      return mockCanonicalMeta(
        a.kind as "movie" | "series",
        a.imdbId as string,
      ) satisfies CanonicalMeta as T;
    case "resolve_sources":
      return mockResolveSources(
        a.kind as "movie" | "series",
        a.imdbId as string,
        a.season as number | undefined,
        a.episode as number | undefined,
      ) satisfies StreamCandidate[] as T;
    case "set_manual_match":
      return undefined as T;
    case "get_episodes":
      return episodesFor(a.seriesId as string) satisfies EpisodesBySeason as T;
    case "get_movie_detail": {
      const found = allMovies().find((m) => m.id === a.movieId);
      if (!found) throw new Error(`devMock: no movie ${a.movieId}`);
      return {
        ...found,
        description:
          "A restless drifter uncovers a conspiracy that reaches further than anyone imagined. (Mock synopsis from the dev backend.)",
        genre: `${found.categoryName}, Adventure`,
        durationSeconds: 5400 + (Number(found.id.replace("movie-", "")) % 8) * 600,
        // Most movies carry a provider backdrop; every 5th has none so the
        // poster-blur fallback path is exercised too (spec §5.4, M18).
        backdropUrl:
          Number(found.id.replace("movie-", "")) % 5 === 0
            ? null
            : svgBackdrop(Number(found.id.replace("movie-", "")), found.name),
      } satisfies MovieDetail as T;
    }
    case "get_series_detail": {
      const found = allSeries().find((s) => s.id === a.seriesId);
      if (!found) throw new Error(`devMock: no series ${a.seriesId}`);
      return {
        ...found,
        description:
          "Each season follows a new cast tangled in the same mystery. (Mock synopsis from the dev backend.)",
        genre: `${found.categoryName}, Mystery`,
        // Series with even index get a backdrop; odd ones fall back to the
        // blurred poster so both hero paths render in the preview (M18).
        backdropUrl:
          Number(found.id.replace("series-", "")) % 2 === 0
            ? svgBackdrop(Number(found.id.replace("series-", "")) + 7, found.name)
            : null,
      } satisfies SeriesDetail as T;
    }
    case "search":
      return mockSearch(a) satisfies SearchResults as T;
    case "test_provider_connection":
      return {
        success: true,
        message: "Mock connection OK.",
        accountInfo: null,
      } satisfies ConnectionTestResult as T;
    case "get_settings":
      return { ...mockSettings } as T;
    case "set_setting": {
      const key = a.key as string;
      const value = a.value as string;
      const camel = key.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
      if (camel === "hwDecodeEnabled") {
        mockSettings.hwDecodeEnabled = value !== "false";
      } else if (camel === "cacheTtlHours" || camel === "imageCacheMaxMb") {
        (mockSettings as unknown as Record<string, number>)[camel] = Number(value);
      } else {
        (mockSettings as unknown as Record<string, unknown>)[camel] = value;
      }
      return undefined as T;
    }
    case "get_related": {
      const ct = a.contentType as string;
      const id = a.contentId as string;
      const limit = (a.limit as number) ?? 20;
      if (ct === "movie") {
        const self = allMovies().find((m) => m.id === id);
        const movies = self
          ? allMovies()
              .filter((m) => m.categoryId === self.categoryId && m.id !== id)
              .slice(0, limit)
          : [];
        return { movies, series: [] } as T;
      }
      if (ct === "series") {
        const self = allSeries().find((s) => s.id === id);
        const series = self
          ? allSeries()
              .filter((s) => s.categoryId === self.categoryId && s.id !== id)
              .slice(0, limit)
          : [];
        return { movies: [], series } as T;
      }
      return { movies: [], series: [] } as T;
    }
    case "resolve_cached_image":
      // No on-disk cache in the browser — always a miss (use the remote URL).
      return null as T;
    case "cache_image":
      return null as T;
    case "image_cache_size":
      return mockImageCacheBytes as T;
    case "clear_image_cache":
      mockImageCacheBytes = 0;
      return undefined as T;
    case "check_provider_status":
      // Browser dev provider is always healthy; the banner is exercised by
      // driving the store directly.
      return {
        reachable: true,
        expired: false,
        message: null,
      } satisfies ProviderStatus as T;
    case "upsert_provider":
      return provider as T;
    case "delete_provider":
      return undefined as T;
    case "resolve_stream_url": {
      mockMpv.lastContentType = a.contentType as string;
      return `mock://stream/${a.contentType}/${a.contentId}` as T;
    }
    case "open_in_external_player":
      console.info("[devMock] external player:", a.streamUrl);
      return undefined as T;
    case "diagnose_playback_failure":
      // Mirror the backend's classified message (Milestone 22). The mock has no
      // real provider to probe, so it returns a representative 403-style reason.
      return ("Provider denied this video (HTTP 403). Live TV is unaffected — VOD may be temporarily restricted by the provider." as unknown) as T;
    case "mpv_load_url":
      mockMpv.load(a.url as string, a.startSeconds as number | undefined);
      return undefined as T;
    case "get_watch_progress":
      return (watchProgress.get(
        wpKey(a.providerId as string, a.contentType as string, a.contentId as string),
      ) ?? null) as T;
    case "set_watch_progress": {
      const position = Math.max(0, Math.round(a.positionSeconds as number));
      const rawDuration = a.durationSeconds as number | null;
      const duration =
        rawDuration && rawDuration > 0 ? Math.round(rawDuration) : null;
      const completed =
        duration !== null && position / duration >= COMPLETION_THRESHOLD;
      watchProgress.set(
        wpKey(a.providerId as string, a.contentType as string, a.contentId as string),
        {
          positionSeconds: position,
          durationSeconds: duration,
          completed,
          updatedAt: Math.floor(Date.now() / 1000),
        },
      );
      return undefined as T;
    }
    case "mark_watched": {
      const rawDuration = a.durationSeconds as number | null;
      const duration =
        rawDuration && rawDuration > 0 ? Math.round(rawDuration) : null;
      watchProgress.set(
        wpKey(a.providerId as string, a.contentType as string, a.contentId as string),
        {
          positionSeconds: duration ?? 0,
          durationSeconds: duration,
          completed: true,
          updatedAt: Math.floor(Date.now() / 1000),
        },
      );
      return undefined as T;
    }
    case "list_watch_progress": {
      // Milestone 39: keyed by "<providerId>:<contentId>" across the scope.
      const ids = scope(a);
      const contentType = a.contentType as string;
      const out: Record<string, WatchProgress> = {};
      for (const [key, value] of watchProgress) {
        const [pid, ct, contentId] = key.split("|");
        if (ct === contentType && ids.includes(pid)) out[`${pid}:${contentId}`] = value;
      }
      return out as T;
    }
    case "clear_watch_progress":
      watchProgress.delete(
        wpKey(a.providerId as string, a.contentType as string, a.contentId as string),
      );
      return undefined as T;
    case "get_continue_watching": {
      const ids = scope(a);
      const limit = Math.min(200, Math.max(1, (a.limit as number) ?? 20));
      const items: ContinueWatchingItem[] = [];
      for (const [k, progress] of watchProgress) {
        const [pid, contentType, contentId] = k.split("|");
        if (!ids.includes(pid) || progress.completed) continue;
        if (contentType === "movie") {
          const movie = allMovies().find((mv) => mv.id === contentId);
          if (movie) items.push({ kind: "movie", movie, progress });
        } else if (contentType === "episode") {
          const found = findEpisodeById(contentId);
          if (found)
            items.push({
              kind: "episode",
              episode: found.episode,
              series: found.series,
              progress,
            });
        }
      }
      items.sort((x, y) => y.progress.updatedAt - x.progress.updatedAt);
      return items.slice(0, limit) as T;
    }

    // --- Custom lists / playlists (Milestone 14) ---
    case "create_list": {
      const nowS = Math.floor(Date.now() / 1000);
      const nextOrder =
        Math.max(-1, ...[...userLists.values()].map((l) => l.sortOrder)) + 1;
      const list: MockList = {
        id: newListId(),
        name: (a.name as string).trim(),
        sortOrder: nextOrder,
        createdAt: nowS,
        updatedAt: nowS,
      };
      userLists.set(list.id, list);
      userListItems.set(list.id, []);
      const out: UserList = {
        id: list.id,
        name: list.name,
        sortOrder: list.sortOrder,
        createdAt: list.createdAt,
        updatedAt: list.updatedAt,
      };
      return out as T;
    }
    case "rename_list": {
      const list = userLists.get(a.listId as string);
      if (list) {
        list.name = (a.name as string).trim();
        list.updatedAt = Math.floor(Date.now() / 1000);
      }
      return undefined as T;
    }
    case "delete_list": {
      userLists.delete(a.listId as string);
      userListItems.delete(a.listId as string);
      return undefined as T;
    }
    case "reorder_lists": {
      (a.orderedListIds as string[]).forEach((id, idx) => {
        const list = userLists.get(id);
        if (list) list.sortOrder = idx;
      });
      return undefined as T;
    }
    case "get_lists": {
      return [...userLists.values()]
        .sort((x, y) => x.sortOrder - y.sortOrder || x.createdAt - y.createdAt)
        .map(listSummary) as T;
    }
    case "add_to_list": {
      const listId = a.listId as string;
      const items = userListItems.get(listId) ?? [];
      const providerId = a.providerId as string;
      const contentType = a.contentType as MockListItem["contentType"];
      const contentId = a.contentId as string;
      if (
        !items.some(
          (i) =>
            i.providerId === providerId &&
            i.contentType === contentType &&
            i.contentId === contentId,
        )
      ) {
        const nextPos = Math.max(-1, ...items.map((i) => i.position)) + 1;
        items.push({ providerId, contentType, contentId, position: nextPos });
        userListItems.set(listId, items);
      }
      const list = userLists.get(listId);
      if (list) list.updatedAt = Math.floor(Date.now() / 1000);
      return undefined as T;
    }
    case "remove_from_list": {
      const listId = a.listId as string;
      const items = userListItems.get(listId) ?? [];
      userListItems.set(
        listId,
        items.filter(
          (i) =>
            !(
              i.providerId === a.providerId &&
              i.contentType === a.contentType &&
              i.contentId === a.contentId
            ),
        ),
      );
      const list = userLists.get(listId);
      if (list) list.updatedAt = Math.floor(Date.now() / 1000);
      return undefined as T;
    }
    case "reorder_list_items": {
      const listId = a.listId as string;
      const items = userListItems.get(listId) ?? [];
      (a.orderedItemKeys as string[]).forEach((key, idx) => {
        const [ct, pid, cid] = key.split(":");
        const it = items.find(
          (i) => i.contentType === ct && i.providerId === pid && i.contentId === cid,
        );
        if (it) it.position = idx;
      });
      return undefined as T;
    }
    case "get_list_items":
      return resolvedItemsFor(a.listId as string) as T;
    case "get_lists_for_item": {
      const providerId = a.providerId as string;
      const contentType = a.contentType as string;
      const contentId = a.contentId as string;
      const out: string[] = [];
      for (const list of userLists.values()) {
        const items = userListItems.get(list.id) ?? [];
        if (
          items.some(
            (i) =>
              i.providerId === providerId &&
              i.contentType === contentType &&
              i.contentId === contentId,
          )
        )
          out.push(list.id);
      }
      return out as T;
    }

    case "mpv_play":
      mockMpv.state = { ...mockMpv.state, paused: false };
      return undefined as T;
    case "mpv_pause":
      mockMpv.state = { ...mockMpv.state, paused: true };
      return undefined as T;
    case "mpv_stop":
      mockMpv.stopTicker();
      mockMpv.state = defaultMpvState();
      return undefined as T;
    case "mpv_seek":
      mockMpv.state = { ...mockMpv.state, position: a.seconds as number };
      return undefined as T;
    case "mpv_set_volume":
      mockMpv.state = { ...mockMpv.state, volume: a.volume as number };
      return undefined as T;
    case "mpv_set_mute":
      mockMpv.state = { ...mockMpv.state, muted: a.muted as boolean };
      return undefined as T;
    case "mpv_set_audio_track":
      mockMpv.state = {
        ...mockMpv.state,
        activeAudioTrack: a.trackId as number,
      };
      return undefined as T;
    case "mpv_set_subtitle_track": {
      const id = a.trackId as number;
      mockMpv.state = {
        ...mockMpv.state,
        activeSubtitleTrack: id < 0 ? null : id,
      };
      return undefined as T;
    }
    case "mpv_get_state":
      return { ...mockMpv.state } as T;
    // --- Multi-view (Milestone 37) ---
    case "mv_add_tile":
      mockMvNextId += 1;
      return mockMvNextId as T;
    case "mv_remove_tile":
    case "mv_set_rects":
    case "mv_set_active_audio":
    case "mv_set_volume":
      return undefined as T;
    case "mv_close":
      mockMvNextId = 0;
      return undefined as T;
    default:
      throw new Error(`devMock: unhandled command "${cmd}"`);
  }
}
