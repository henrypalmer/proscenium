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
  LiveChannel,
  PaginatedResult,
  Provider,
} from "../types";

const LATENCY_MS = 350;
const CHANNEL_COUNT = 12_000;

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
        movies: 0,
        series: 0,
      } satisfies CatalogSummary as T;
    case "get_live_categories":
      return CATEGORY_NAMES.map((name, i) => ({
        id: name,
        name,
        sortOrder: i,
      })) satisfies Category[] as T;
    case "get_live_channels": {
      const categoryId = a.categoryId as string | undefined;
      const page = Math.max(1, (a.page as number) ?? 1);
      const pageSize = Math.min(500, Math.max(1, (a.pageSize as number) ?? 200));
      const filtered = allChannels()
        .filter((c) => !categoryId || c.categoryId === categoryId)
        .sort((x, y) => x.name.toLowerCase().localeCompare(y.name.toLowerCase()));
      const start = (page - 1) * pageSize;
      return {
        items: filtered.slice(start, start + pageSize),
        total: filtered.length,
        page,
        pageSize,
      } satisfies PaginatedResult<LiveChannel> as T;
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
    default:
      throw new Error(`devMock: unhandled command "${cmd}"`);
  }
}
