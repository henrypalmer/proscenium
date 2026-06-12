/**
 * Browser-only mock backend. When the app runs outside the Tauri shell
 * (`npm run dev` in a plain browser) every invoke() is served from here so
 * the UI can be developed and exercised without the Rust backend.
 * Behavior mirrors the real commands: pagination, category filtering, and
 * case-insensitive alphabetical ordering.
 */

import type {
  CatalogSummary,
  Category,
  ConnectionTestResult,
  Episode,
  EpisodesBySeason,
  LiveChannel,
  Movie,
  MovieDetail,
  MpvState,
  PaginatedResult,
  Provider,
  Series,
  SeriesDetail,
} from "../types";

const LATENCY_MS = 350;
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
      return {
        id: `live-${i}`,
        name: `${FIRST[i % FIRST.length]} ${SECOND[i % SECOND.length]} ${String(i % 997).padStart(3, "0")}`,
        categoryId: category,
        categoryName: category,
        logoUrl,
        streamUrl: `http://mock.local/live/${i}.ts`,
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

// --- Mock VOD catalog (Milestone 5) ---

const MOVIE_GENRES = [
  "Action", "Comedy", "Drama", "Documentary", "Horror", "Sci-Fi", "Thriller",
  "Romance", "Animation", "Family", "Crime", "Adventure", "Fantasy",
  "Mystery", "War", "Western",
];
const SERIES_GENRES = [
  "Crime", "Drama", "Comedy", "Sci-Fi", "Fantasy", "Reality", "Kids",
  "Documentary", "Anime", "Classic",
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

function posterFor(i: number, title: string): string | null {
  return i % 3 === 0
    ? svgPoster(i, title)
    : i % 3 === 1
      ? `http://invalid.local/poster-${i}.jpg`
      : null;
}

let movieCache: Movie[] | null = null;
function allMovies(): Movie[] {
  if (!movieCache) {
    movieCache = Array.from({ length: MOVIE_COUNT }, (_, i) => {
      const genre = MOVIE_GENRES[i % MOVIE_GENRES.length];
      const name = `${TITLE_A[i % TITLE_A.length]} ${TITLE_B[(i * 7) % TITLE_B.length]} ${String(i % 887).padStart(3, "0")}`;
      return {
        id: `movie-${i}`,
        name,
        categoryId: genre,
        categoryName: genre,
        posterUrl: posterFor(i, name),
        streamUrl: `http://mock.local/movie/${i}.mp4`,
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
    grouped[s] = Array.from({ length: count }, (_, e): Episode => ({
      id: `ep-${n}-${s}-${e + 1}`,
      seriesId,
      season: s,
      episode: e + 1,
      title: `S${String(s).padStart(2, "0")}E${String(e + 1).padStart(2, "0")} — ${TITLE_B[(n + s + e) % TITLE_B.length]}`,
      streamUrl: `http://mock.local/series/${n}/${s}/${e + 1}.mp4`,
      containerExt: "mp4",
      durationSeconds: 1260 + ((n + e) % 5) * 300,
      posterUrl: null,
    }));
  }
  return grouped;
}

/** Shared pagination: case-insensitive alphabetical, optional genre filter
 * (mirrors the backend's ORDER BY name COLLATE NOCASE). */
function paginateByName<T extends { name: string; categoryId: string }>(
  list: T[],
  a: Args,
): PaginatedResult<T> {
  const categoryId = a.categoryId as string | undefined;
  const page = Math.max(1, (a.page as number) ?? 1);
  const pageSize = Math.min(500, Math.max(1, (a.pageSize as number) ?? 200));
  const filtered = list
    .filter((item) => !categoryId || item.categoryId === categoryId)
    .sort((x, y) => x.name.toLowerCase().localeCompare(y.name.toLowerCase()));
  const start = (page - 1) * pageSize;
  return {
    items: filtered.slice(start, start + pageSize),
    total: filtered.length,
    page,
    pageSize,
  };
}

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

  load(url: string) {
    this.stopTicker();
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

export async function mockInvoke<T>(cmd: string, args?: unknown): Promise<T> {
  await sleep(LATENCY_MS);
  const a = (args ?? {}) as Args;
  switch (cmd) {
    case "list_providers":
      return [provider] as T;
    case "get_active_provider":
      return provider as T;
    case "set_active_provider":
    case "refresh_catalog":
      return undefined as T;
    case "get_catalog_summary":
      return {
        liveChannels: CHANNEL_COUNT,
        movies: MOVIE_COUNT,
        series: SERIES_COUNT,
      } satisfies CatalogSummary as T;
    case "get_live_categories":
      return CATEGORY_NAMES.map((name, i) => ({
        id: name,
        name,
        sortOrder: i,
      })) satisfies Category[] as T;
    case "get_live_channels":
      return paginateByName(allChannels(), a) satisfies PaginatedResult<LiveChannel> as T;
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
      } satisfies SeriesDetail as T;
    }
    case "test_provider_connection":
      return {
        success: true,
        message: "Mock connection OK.",
        accountInfo: null,
      } satisfies ConnectionTestResult as T;
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
    case "mpv_load_url":
      mockMpv.load(a.url as string);
      return undefined as T;
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
    default:
      throw new Error(`devMock: unhandled command "${cmd}"`);
  }
}
