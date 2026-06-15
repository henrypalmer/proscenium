import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import * as api from "../lib/tauri";
import type { LiveChannel, Movie, PaginatedResult, Series } from "../types";

export const PAGE_SIZE = 200;

type PageFetcher<T> = (
  providerId: string,
  categoryId: string | undefined,
  page: number,
  pageSize: number,
) => Promise<PaginatedResult<T>>;

/**
 * Sparse, page-on-demand item loading for virtualized lists/grids. Items are
 * fetched in PAGE_SIZE chunks as the visible range reaches them; `getItem`
 * returns undefined for rows whose page hasn't landed yet (rendered as
 * skeletons).
 */
export function usePagedItems<T>(
  providerId: string | null,
  categoryId: string | null,
  version: number,
  fetchPage: PageFetcher<T>,
) {
  const [total, setTotal] = useState<number | null>(null);
  const [, bump] = useReducer((x: number) => x + 1, 0);
  const items = useRef<(T | undefined)[]>([]);
  const pages = useRef<Map<number, "loading" | "done">>(new Map());
  // Invalidates in-flight responses from a previous provider/category.
  const generation = useRef(0);

  const loadPage = useCallback(
    async (page: number) => {
      if (!providerId || pages.current.has(page)) return;
      pages.current.set(page, "loading");
      const gen = generation.current;
      try {
        const result = await fetchPage(
          providerId,
          categoryId ?? undefined,
          page,
          PAGE_SIZE,
        );
        if (gen !== generation.current) return;
        pages.current.set(page, "done");
        const offset = (page - 1) * PAGE_SIZE;
        for (let i = 0; i < result.items.length; i++) {
          items.current[offset + i] = result.items[i];
        }
        setTotal(result.total);
        bump();
      } catch {
        if (gen === generation.current) pages.current.delete(page);
      }
    },
    [providerId, categoryId, fetchPage],
  );

  useEffect(() => {
    generation.current += 1;
    items.current = [];
    pages.current = new Map();
    setTotal(null);
    void loadPage(1);
  }, [loadPage, version]);

  const ensureRange = useCallback(
    (startIndex: number, endIndex: number) => {
      const first = Math.floor(startIndex / PAGE_SIZE) + 1;
      const last = Math.floor(endIndex / PAGE_SIZE) + 1;
      for (let page = first; page <= last; page++) {
        void loadPage(page);
      }
    },
    [loadPage],
  );

  return {
    total,
    getItem: (index: number) => items.current[index],
    ensureRange,
  };
}

export function usePagedLiveChannels(
  providerId: string | null,
  categoryId: string | null,
  version: number,
  query?: string,
) {
  // Fold the in-section filter (spec §5.3) into the fetcher identity: a new
  // `query` produces a new fetcher, which resets the paged list and refetches
  // from page 1 against the whole (filtered) category.
  const fetchPage = useCallback<PageFetcher<LiveChannel>>(
    (providerId, categoryId, page, pageSize) =>
      api.getLiveChannels(providerId, categoryId, query || undefined, page, pageSize),
    [query],
  );
  return usePagedItems<LiveChannel>(providerId, categoryId, version, fetchPage);
}

export function usePagedMovies(
  providerId: string | null,
  categoryId: string | null,
  version: number,
) {
  return usePagedItems<Movie>(providerId, categoryId, version, api.getMovies);
}

export function usePagedSeries(
  providerId: string | null,
  categoryId: string | null,
  version: number,
) {
  return usePagedItems<Series>(providerId, categoryId, version, api.getSeries);
}
