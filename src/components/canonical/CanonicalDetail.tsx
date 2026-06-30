import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../../lib/tauri";
import { usePlayerStore } from "../../store/playerStore";
import CachedImage from "../common/CachedImage";
import HeroBackdrop from "../vod/HeroBackdrop";
import { Poster } from "../vod/PosterGrid";
import SeasonSelect from "../vod/SeasonSelect";
import type { CanonicalItem, CanonicalMeta, CanonicalVideo } from "../../types";

interface Props {
  /** Base card — rendered immediately while the full meta loads. */
  item: CanonicalItem;
  onClose: () => void;
}

/**
 * Canonical (Cinemeta) detail overlay (M40 slice 1 — browse-only): hero
 * backdrop, poster, overview, genres/rating and, for series, the season/episode
 * list. Resolving playback sources across providers + a Play/source picker
 * arrive in M40 slice 3 (the placeholder below marks where they land).
 */
export default function CanonicalDetail({ item, onClose }: Props) {
  const [meta, setMeta] = useState<CanonicalMeta | null>(null);
  const [season, setSeason] = useState<number | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    setMeta(null);
    setSeason(null);
    void (async () => {
      try {
        const m = await api.getCanonicalMeta(item.kind, item.imdbId);
        if (!cancelled) setMeta(m);
      } catch {
        // Browse-only; the base card already rendered the poster/title.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [item.kind, item.imdbId]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !usePlayerStore.getState().open) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Episodes grouped by season (series only).
  const bySeason = useMemo(() => {
    const map = new Map<number, CanonicalVideo[]>();
    for (const v of meta?.videos ?? []) {
      const arr = map.get(v.season) ?? [];
      arr.push(v);
      map.set(v.season, arr);
    }
    return map;
  }, [meta]);
  const seasons = useMemo(
    () => [...bySeason.keys()].sort((a, b) => a - b),
    [bySeason],
  );
  useEffect(() => {
    if (season === null && seasons.length > 0) setSeason(seasons[0]);
  }, [seasons, season]);

  const genres = meta?.genres ?? [];
  const year =
    meta?.releaseInfo ?? (item.releaseYear ? String(item.releaseYear) : "");

  return (
    <div
      ref={scrollRef}
      data-testid="canonical-detail"
      className="absolute inset-0 z-20 overflow-y-auto bg-zinc-950"
    >
      <HeroBackdrop backdropUrl={meta?.backdropUrl ?? null} posterUrl={item.posterUrl} />
      <div className="relative mx-auto max-w-5xl px-6 pb-12">
        <button
          onClick={onClose}
          data-testid="detail-back"
          className="mt-4 rounded-md bg-zinc-950/40 px-2 py-1 text-sm text-zinc-300 backdrop-blur-sm hover:bg-zinc-900 hover:text-zinc-100"
        >
          ← Back
        </button>
        <div className="flex items-end gap-6 pt-[140px]">
          <div className="w-40 shrink-0 drop-shadow-2xl sm:w-52">
            <Poster url={item.posterUrl} title={item.name} vtName="vt-poster" />
          </div>
          <div className="min-w-0 flex-1 pb-1">
            <h1 className="text-3xl font-bold text-white drop-shadow-md">{item.name}</h1>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-zinc-300">
              {year && <span>{year}</span>}
              {meta?.runtime && <span>· {meta.runtime}</span>}
              {meta?.imdbRating != null && <span>· ★ {meta.imdbRating.toFixed(1)}</span>}
            </div>
            <div className="mt-3 flex flex-wrap gap-1.5">
              {genres.map((g) => (
                <span
                  key={g}
                  className="rounded-full bg-zinc-800/80 px-2.5 py-0.5 text-xs text-zinc-200 backdrop-blur-sm"
                >
                  {g}
                </span>
              ))}
            </div>
            {/* Source resolution + Play/source picker arrive in M40 slice 3. */}
            <div className="mt-6">
              <span
                data-testid="canonical-sources-pending"
                className="inline-block rounded-md border border-zinc-700 bg-zinc-900/60 px-4 py-2 text-xs text-zinc-400"
              >
                Finding sources across your providers is coming next.
              </span>
            </div>
          </div>
        </div>

        {meta?.description && (
          <p className="prosc-enter mt-6 max-w-3xl text-[15px] leading-relaxed text-zinc-300">
            {meta.description}
          </p>
        )}

        {item.kind === "series" && (
          <div className="mt-10">
            <h2 className="mb-3 text-lg font-semibold text-zinc-100">Episodes</h2>
            {meta === null ? (
              <div className="space-y-2">
                <div className="h-8 w-64 animate-pulse rounded bg-zinc-900" />
                <div className="h-16 w-full animate-pulse rounded bg-zinc-900" />
                <div className="h-16 w-full animate-pulse rounded bg-zinc-900" />
              </div>
            ) : seasons.length === 0 ? (
              <p className="text-sm text-zinc-500">
                No episode information is available for this title.
              </p>
            ) : (
              <div className="prosc-enter">
                <SeasonSelect
                  seasons={seasons}
                  value={season ?? seasons[0]}
                  onChange={setSeason}
                />
                <ul className="mt-4 space-y-2">
                  {(bySeason.get(season ?? seasons[0]) ?? []).map((ep) => (
                    <li key={ep.id} className="flex gap-3 rounded-lg bg-zinc-900/50 p-2">
                      <div className="relative aspect-video w-32 shrink-0 overflow-hidden rounded bg-zinc-800">
                        <CachedImage
                          url={ep.thumbnail}
                          className="absolute inset-0 h-full w-full object-cover"
                        />
                      </div>
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-medium text-zinc-200">
                          S{ep.season}:E{ep.episode} · {ep.name}
                        </p>
                        {ep.overview && (
                          <p className="mt-1 line-clamp-2 text-xs text-zinc-500">
                            {ep.overview}
                          </p>
                        )}
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
