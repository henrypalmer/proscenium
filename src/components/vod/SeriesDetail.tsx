import { useEffect, useState } from "react";
import * as api from "../../lib/tauri";
import { useCatalogStore } from "../../store/catalogStore";
import { usePlayerStore } from "../../store/playerStore";
import EpisodeList from "./EpisodeList";
import { Poster } from "./PosterGrid";
import type {
  Episode,
  EpisodesBySeason,
  Series,
  SeriesDetail as SeriesDetailData,
} from "../../types";

interface SeriesDetailProps {
  providerId: string;
  /** Base row from the grid — rendered immediately while metadata loads. */
  series: Series;
  onClose: () => void;
}

/**
 * Series detail panel (spec §5.4): poster, metadata, season selector and
 * `EpisodeList`. The detail fetch runs first so Xtream episodes persisted by
 * it are served from the cache when the episode fetch follows.
 */
export default function SeriesDetail({
  providerId,
  series,
  onClose,
}: SeriesDetailProps) {
  const notify = useCatalogStore((s) => s.notify);
  const [detail, setDetail] = useState<SeriesDetailData | null>(null);
  const [episodes, setEpisodes] = useState<EpisodesBySeason | null>(null);
  const [episodesError, setEpisodesError] = useState<string | null>(null);
  const [season, setSeason] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const d = await api.getSeriesDetail(providerId, series.id);
        if (!cancelled) setDetail(d);
      } catch {
        // Metadata is optional; the base row already renders.
      }
      try {
        const grouped = await api.getEpisodes(providerId, series.id);
        if (cancelled) return;
        setEpisodes(grouped);
        const seasons = Object.keys(grouped)
          .map(Number)
          .sort((a, b) => a - b);
        setSeason(seasons[0] ?? null);
      } catch (e) {
        if (!cancelled) setEpisodesError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [providerId, series.id]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // The player overlay owns Esc while it is open.
      if (e.key === "Escape" && !usePlayerStore.getState().open) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const play = (episode: Episode) =>
    void usePlayerStore.getState().openContent({
      providerId,
      contentType: "episode",
      contentId: episode.id,
      title: `${series.name} — ${episode.title}`,
    });

  const openExternal = async (episode: Episode) => {
    try {
      const url = await api.resolveStreamUrl(providerId, "episode", episode.id);
      await api.openInExternalPlayer(url);
    } catch (e) {
      notify(String(e), "error");
    }
  };

  const genres = (detail?.genre ?? series.categoryName)
    .split(",")
    .map((g) => g.trim())
    .filter(Boolean);
  const seasons =
    episodes === null
      ? []
      : Object.keys(episodes)
          .map(Number)
          .sort((a, b) => a - b);

  return (
    <div
      data-testid="series-detail"
      className="absolute inset-0 z-20 overflow-y-auto bg-zinc-950"
    >
      <div className="mx-auto max-w-4xl p-6">
        <button
          onClick={onClose}
          data-testid="detail-back"
          className="mb-4 rounded-md px-2 py-1 text-sm text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
        >
          ← Back
        </button>
        <div className="flex gap-6">
          <div className="w-48 shrink-0 sm:w-56">
            <Poster url={series.posterUrl} title={series.name} />
          </div>
          <div className="min-w-0 flex-1">
            <h1 className="text-2xl font-semibold text-white">{series.name}</h1>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-zinc-400">
              {series.releaseYear && <span>{series.releaseYear}</span>}
            </div>
            <div className="mt-3 flex flex-wrap gap-1.5">
              {genres.map((genre) => (
                <span
                  key={genre}
                  className="rounded-full bg-zinc-800 px-2.5 py-0.5 text-xs text-zinc-300"
                >
                  {genre}
                </span>
              ))}
            </div>
            {detail === null ? (
              <div className="mt-6 space-y-2">
                <div className="h-3 w-full animate-pulse rounded bg-zinc-900" />
                <div className="h-3 w-5/6 animate-pulse rounded bg-zinc-900" />
              </div>
            ) : (
              detail.description && (
                <p className="mt-6 max-w-prose text-sm leading-relaxed text-zinc-400">
                  {detail.description}
                </p>
              )
            )}
          </div>
        </div>

        <div className="mt-8">
          {episodesError !== null ? (
            <p className="text-sm text-red-400">
              Could not load episodes: {episodesError}
            </p>
          ) : episodes === null ? (
            <div className="space-y-2">
              <div className="h-8 w-64 animate-pulse rounded bg-zinc-900" />
              <div className="h-10 w-full animate-pulse rounded bg-zinc-900" />
              <div className="h-10 w-full animate-pulse rounded bg-zinc-900" />
            </div>
          ) : seasons.length === 0 ? (
            <p className="text-sm text-zinc-500">
              No episodes are available for this series.
            </p>
          ) : (
            <>
              <div
                data-testid="season-selector"
                className="flex flex-wrap gap-1.5"
              >
                {seasons.map((s) => (
                  <button
                    key={s}
                    onClick={() => setSeason(s)}
                    data-testid="season-tab"
                    className={`rounded-md px-3 py-1.5 text-sm transition-colors ${
                      season === s
                        ? "bg-zinc-100 font-semibold text-zinc-900"
                        : "bg-zinc-900 text-zinc-300 hover:bg-zinc-800"
                    }`}
                  >
                    Season {s}
                  </button>
                ))}
              </div>
              {season !== null && episodes[season] && (
                <div className="mt-4">
                  <EpisodeList
                    episodes={episodes[season]}
                    onPlay={play}
                    onOpenExternal={(e) => void openExternal(e)}
                  />
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
