import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../../lib/tauri";
import { episodeLabel } from "../../lib/utils";
import { useCatalogStore } from "../../store/catalogStore";
import { usePlayerStore } from "../../store/playerStore";
import { useProgressStore } from "../../store/progressStore";
import AddToListMenu from "../lists/AddToListMenu";
import EpisodeList from "./EpisodeList";
import HeroBackdrop from "./HeroBackdrop";
import { Poster } from "./PosterGrid";
import SeasonSelect from "./SeasonSelect";
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
  const [addTo, setAddTo] = useState<{ x: number; y: number } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const d = await api.getSeriesDetail(providerId, series.id);
        if (!cancelled) setDetail(d);
      } catch {
        // Metadata is optional; the base row already renders.
      }
      // Watch progress for episode rows (spec §5.9).
      void useProgressStore.getState().loadSection(providerId, "episode");
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
      title: episodeLabel(series.name, episode.season, episode.episode, episode.title),
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

  // Top-level Play/Resume CTA (spec §5.4 / Milestone 26): resume the most-recent
  // in-progress episode, else play the first episode — without scrolling to the
  // list. Reuses the §5.9 progress cache loaded above for the episode rows.
  const progressEntries = useProgressStore((s) => s.entries);
  const ctaTarget = useMemo(() => {
    if (!episodes) return null;
    const all = Object.values(episodes).flat();
    if (all.length === 0) return null;
    let resume: Episode | null = null;
    let resumeAt = -1;
    for (const ep of all) {
      const p = progressEntries[`${providerId}|episode|${ep.id}`];
      if (p && !p.completed && p.positionSeconds >= 5 && p.updatedAt > resumeAt) {
        resume = ep;
        resumeAt = p.updatedAt;
      }
    }
    if (resume) return { episode: resume, isResume: true };
    const first = [...all].sort((a, b) => a.season - b.season || a.episode - b.episode)[0];
    return { episode: first, isResume: false };
  }, [episodes, progressEntries, providerId]);

  const playCta = () => {
    if (!ctaTarget) return;
    const { episode, isResume } = ctaTarget;
    const args = {
      providerId,
      contentType: "episode" as const,
      contentId: episode.id,
      title: episodeLabel(series.name, episode.season, episode.episode, episode.title),
    };
    if (isResume) {
      const p = progressEntries[`${providerId}|episode|${episode.id}`];
      void usePlayerStore.getState().playDirect(args, p?.positionSeconds ?? 0);
    } else {
      void usePlayerStore.getState().openContent(args);
    }
  };

  return (
    <div
      ref={scrollRef}
      data-testid="series-detail"
      className="absolute inset-0 z-20 overflow-y-auto bg-zinc-950"
    >
      <HeroBackdrop backdropUrl={detail?.backdropUrl ?? null} posterUrl={series.posterUrl} />
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
            <Poster url={series.posterUrl} title={series.name} vtName="vt-poster" />
          </div>
          <div className="min-w-0 flex-1 pb-1">
            <h1 className="text-3xl font-bold text-white drop-shadow-md">
              {series.name}
            </h1>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-zinc-300">
              {series.releaseYear && <span>{series.releaseYear}</span>}
            </div>
            <div className="mt-3 flex flex-wrap gap-1.5">
              {genres.map((genre) => (
                <span
                  key={genre}
                  className="rounded-full bg-zinc-800/80 px-2.5 py-0.5 text-xs text-zinc-200 backdrop-blur-sm"
                >
                  {genre}
                </span>
              ))}
            </div>
            <div className="mt-6 flex flex-wrap gap-3">
              {ctaTarget && (
                <button
                  onClick={playCta}
                  data-testid="series-cta"
                  className="rounded-md bg-zinc-100 px-5 py-2 text-sm font-semibold text-zinc-900 hover:bg-white"
                >
                  ▶ {ctaTarget.isResume ? "Resume" : "Play"} S{ctaTarget.episode.season}:E
                  {ctaTarget.episode.episode}
                </button>
              )}
              <button
                onClick={(e) => {
                  const r = e.currentTarget.getBoundingClientRect();
                  setAddTo({ x: r.left, y: r.bottom });
                }}
                data-testid="detail-add-to-list"
                className="rounded-md border border-zinc-600 bg-zinc-950/40 px-5 py-2 text-sm font-medium text-zinc-100 backdrop-blur-sm hover:bg-zinc-900"
              >
                + Add to list
              </button>
            </div>
          </div>
        </div>

        {detail !== null && detail.description && (
          <p className="prosc-enter mt-6 max-w-3xl text-[15px] leading-relaxed text-zinc-300">
            {detail.description}
          </p>
        )}

        <div className="mt-10">
          <h2 className="mb-3 text-lg font-semibold text-zinc-100">Episodes</h2>
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
            <div className="prosc-enter">
              <SeasonSelect
                seasons={seasons}
                value={season ?? seasons[0]}
                onChange={setSeason}
              />
              {season !== null && episodes[season] && (
                <div className="mt-4">
                  <EpisodeList
                    providerId={providerId}
                    seriesName={series.name}
                    episodes={episodes[season]}
                    scrollRef={scrollRef}
                    onPlay={play}
                    onOpenExternal={(e) => void openExternal(e)}
                  />
                </div>
              )}
            </div>
          )}
        </div>
      </div>
      {addTo && (
        <AddToListMenu
          providerId={providerId}
          contentType="series"
          contentId={series.id}
          x={addTo.x}
          y={addTo.y}
          onClose={() => setAddTo(null)}
        />
      )}
    </div>
  );
}
