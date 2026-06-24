import { useVirtualizer } from "@tanstack/react-virtual";
import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { cleanEpisodeTitle, formatDuration } from "../../lib/utils";
import { useWatchProgress } from "../../store/progressStore";
import ContextMenu, { type ContextMenuItem } from "../common/ContextMenu";
import Placeholder from "../common/Placeholder";
import WatchProgressOverlay from "./WatchProgressOverlay";
import type { Episode, WatchProgress } from "../../types";

/** Position past which an episode counts as resumable (matches the §5.9 prompt
 * threshold): anything shorter just starts over, so the menu says "Play". */
const RESUME_THRESHOLD_SECONDS = 5;

function isResumable(progress: WatchProgress | undefined): boolean {
  return (
    !!progress &&
    !progress.completed &&
    progress.positionSeconds > RESUME_THRESHOLD_SECONDS
  );
}

interface EpisodeListProps {
  providerId: string;
  /** Parent series name, used to strip redundant title prefixes (§5.4, M20). */
  seriesName: string;
  episodes: Episode[];
  /** The series-detail scroll container; the list is windowed against it. */
  scrollRef: RefObject<HTMLElement | null>;
  onPlay: (episode: Episode) => void;
  onOpenExternal: (episode: Episode) => void;
}

interface MenuState {
  episode: Episode;
  resumable: boolean;
  x: number;
  y: number;
}

/**
 * Episodes for the selected season (spec §5.4, M20): each row leads with the
 * episode thumbnail (click to play/resume via the §5.9 flow), with a clean
 * title, an "Episode N · duration" line, and a short synopsis to the right.
 * Play / Open in External Player live in a right-click (and hover "⋯") context
 * menu. The list is virtualized against the page scroll container (§10) since a
 * single "season" can hold hundreds of episodes for daily content.
 */
export default function EpisodeList({
  providerId,
  seriesName,
  episodes,
  scrollRef,
  onPlay,
  onOpenExternal,
}: EpisodeListProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);
  const [menu, setMenu] = useState<MenuState | null>(null);

  // The list shares the detail page's scroll, so the virtualizer needs the
  // list's offset within that scroll content (`scrollMargin`). Content above
  // it (hero, synopsis) is stable by the time episodes load, but re-measure on
  // resize / season change to stay correct.
  useLayoutEffect(() => {
    const scrollEl = scrollRef.current;
    const listEl = listRef.current;
    if (!scrollEl || !listEl) return;
    const measure = () => {
      setScrollMargin(
        listEl.getBoundingClientRect().top -
          scrollEl.getBoundingClientRect().top +
          scrollEl.scrollTop,
      );
    };
    measure();
    const raf = requestAnimationFrame(measure);
    const ro = new ResizeObserver(measure);
    ro.observe(scrollEl);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, [scrollRef, episodes]);

  const virtualizer = useVirtualizer({
    count: episodes.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 116,
    overscan: 6,
    scrollMargin,
  });
  const virtualRows = virtualizer.getVirtualItems();

  const closeMenu = () => setMenu(null);

  const menuItems: ContextMenuItem[] = menu
    ? [
        {
          label: menu.resumable ? "Resume" : "Play",
          onSelect: () => onPlay(menu.episode),
        },
        {
          label: "Open in External Player",
          onSelect: () => onOpenExternal(menu.episode),
        },
      ]
    : [];

  return (
    <div ref={listRef} data-testid="episode-list">
      <div
        style={{ height: virtualizer.getTotalSize(), position: "relative" }}
      >
        {virtualRows.map((row) => {
          const episode = episodes[row.index];
          return (
            <div
              key={episode.id}
              data-index={row.index}
              ref={virtualizer.measureElement}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${row.start - scrollMargin}px)`,
              }}
            >
              <EpisodeRow
                providerId={providerId}
                seriesName={seriesName}
                episode={episode}
                onPlay={onPlay}
                onMenu={(ep, resumable, x, y) =>
                  setMenu({ episode: ep, resumable, x, y })
                }
              />
            </div>
          );
        })}
      </div>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems}
          onClose={closeMenu}
        />
      )}
    </div>
  );
}

function EpisodeRow({
  providerId,
  seriesName,
  episode,
  onPlay,
  onMenu,
}: {
  providerId: string;
  seriesName: string;
  episode: Episode;
  onPlay: (episode: Episode) => void;
  onMenu: (
    episode: Episode,
    resumable: boolean,
    x: number,
    y: number,
  ) => void;
}) {
  const progress = useWatchProgress(providerId, "episode", episode.id);
  const title = cleanEpisodeTitle(seriesName, episode.episode, episode.title);
  const meta = [
    `Episode ${episode.episode}`,
    episode.durationSeconds !== null
      ? formatDuration(episode.durationSeconds)
      : null,
  ]
    .filter(Boolean)
    .join(" · ");

  const openMenu = (x: number, y: number) =>
    onMenu(episode, isResumable(progress), x, y);

  return (
    <div
      data-testid="episode-row"
      onContextMenu={(e) => {
        e.preventDefault();
        openMenu(e.clientX, e.clientY);
      }}
      className="group relative flex gap-4 rounded-lg p-2 hover:bg-zinc-900/60"
    >
      <EpisodeThumbnail
        episode={episode}
        title={title}
        progress={progress}
        onPlay={() => onPlay(episode)}
      />
      <div className="min-w-0 flex-1 py-0.5">
        <p className="truncate text-sm font-semibold text-zinc-100">{title}</p>
        <p className="mt-0.5 text-xs tabular-nums text-zinc-500">{meta}</p>
        {episode.overview && (
          <p className="mt-1.5 line-clamp-2 text-sm leading-snug text-zinc-400">
            {episode.overview}
          </p>
        )}
      </div>
      <button
        type="button"
        aria-label="More options"
        data-testid="episode-menu-button"
        onClick={(e) => {
          e.stopPropagation();
          const r = e.currentTarget.getBoundingClientRect();
          openMenu(r.right, r.bottom);
        }}
        className="absolute right-2 top-2 rounded-md bg-black/50 px-1.5 py-0.5 text-sm leading-none text-zinc-100 opacity-0 backdrop-blur transition hover:bg-black/80 focus-visible:opacity-100 group-hover:opacity-100"
      >
        ⋯
      </button>
    </div>
  );
}

/** 16:9 episode thumbnail; clicking plays/resumes the episode (§5.9). The
 * watched check and progress bar overlay the art via `WatchProgressOverlay`. */
function EpisodeThumbnail({
  episode,
  title,
  progress,
  onPlay,
}: {
  episode: Episode;
  title: string;
  progress: WatchProgress | undefined;
  onPlay: () => void;
}) {
  const [state, setState] = useState<"loading" | "loaded" | "error">(
    episode.posterUrl ? "loading" : "error",
  );
  return (
    <button
      type="button"
      onClick={onPlay}
      data-testid="episode-play"
      title={`Play ${title}`}
      className="relative aspect-video w-40 shrink-0 overflow-hidden rounded-md bg-zinc-800 sm:w-44"
    >
      <Placeholder label={title} />
      {episode.posterUrl && state !== "error" && (
        <img
          src={episode.posterUrl}
          alt=""
          loading="lazy"
          decoding="async"
          onLoad={() => setState("loaded")}
          onError={() => setState("error")}
          className={`absolute inset-0 h-full w-full object-cover transition-opacity duration-150 ${
            state === "loaded" ? "opacity-100" : "opacity-0"
          }`}
        />
      )}
      {/* Play affordance on hover. */}
      <span className="absolute inset-0 flex items-center justify-center bg-black/0 text-2xl text-white/0 transition group-hover:bg-black/30 group-hover:text-white/90">
        ▶
      </span>
      <WatchProgressOverlay progress={progress} />
    </button>
  );
}
