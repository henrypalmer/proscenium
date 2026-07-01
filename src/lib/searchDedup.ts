import * as api from "./tauri";
import type {
  CanonicalItem,
  CanonicalSearchResults,
  DedupCanonical,
  DedupProviderHit,
  Movie,
  SearchResults,
  Series,
} from "../types";

/** The `providerId:id` key a provider hit is rendered under (the group getKey). */
export const providerHitKey = (item: Movie | Series): string =>
  `${item.providerId}:${item.id}`;

const toCanonical = (items: CanonicalItem[]): DedupCanonical[] =>
  items.map((i) => ({ imdbId: i.imdbId, name: i.name, year: i.releaseYear }));

const toProvider = (items: (Movie | Series)[]): DedupProviderHit[] =>
  items.map((i) => ({
    key: providerHitKey(i),
    providerId: i.providerId,
    contentId: i.id,
    name: i.name,
    year: i.releaseYear,
  }));

/**
 * The set of provider search-hit keys that duplicate a canonical ("All Sources")
 * hit and should be hidden from the provider groups (M44). Movies and series are
 * deduped independently against their canonical counterparts. Empty on any
 * failure (dedup never removes a hit when the backend can't confirm it), and
 * empty when either side is missing — so it applies only once both are in.
 */
export async function computeSearchHideKeys(
  results: SearchResults | null,
  canonical: CanonicalSearchResults | null,
): Promise<Set<string>> {
  if (!results || !canonical) return new Set();
  try {
    const [movieHide, seriesHide] = await Promise.all([
      results.movies.length && canonical.movies.length
        ? api.dedupSearchHits("movie", toCanonical(canonical.movies), toProvider(results.movies))
        : Promise.resolve<string[]>([]),
      results.series.length && canonical.series.length
        ? api.dedupSearchHits("series", toCanonical(canonical.series), toProvider(results.series))
        : Promise.resolve<string[]>([]),
    ]);
    return new Set([...movieHide, ...seriesHide]);
  } catch {
    return new Set();
  }
}

/** Apply a hide-set to the provider results, dropping deduped movie/series hits.
 * Live TV is never deduped. Returns `results` unchanged when nothing is hidden. */
export function applyHideKeys(
  results: SearchResults | null,
  hide: Set<string>,
): SearchResults | null {
  if (!results || hide.size === 0) return results;
  return {
    liveChannels: results.liveChannels,
    movies: results.movies.filter((m) => !hide.has(providerHitKey(m))),
    series: results.series.filter((s) => !hide.has(providerHitKey(s))),
  };
}
