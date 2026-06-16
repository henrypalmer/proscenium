import { Poster } from "../vod/PosterGrid";
import WatchProgressOverlay from "../vod/WatchProgressOverlay";
import type { ContinueWatchingItem } from "../../types";

interface KeepWatchingCardProps {
  item: ContinueWatchingItem;
  onActivate: (item: ContinueWatchingItem) => void;
  /** Open the card's menu (Mark as watched / Remove from list, §5.10). */
  onMenu: (item: ContinueWatchingItem, x: number, y: number) => void;
}

/** Title / subtitle / poster for a Keep Watching entry. Episodes read as their
 * series: the series poster and title are used so a show in progress looks like
 * the show, not a single episode (spec §5.10). When the parent series is
 * unknown (e.g. a catalog-orphaned episode) we fall back to the episode's own
 * image/title. */
function describe(item: ContinueWatchingItem) {
  if (item.kind === "movie") {
    return {
      title: item.movie.name,
      subtitle: item.movie.releaseYear ? String(item.movie.releaseYear) : "",
      poster: item.movie.posterUrl,
    };
  }
  const { episode, series } = item;
  const tag = `S${episode.season}·E${episode.episode}`;
  return {
    title: series?.name ?? episode.title,
    subtitle: series ? `${tag} · ${episode.title}` : tag,
    poster: series?.posterUrl ?? episode.posterUrl ?? null,
  };
}

/**
 * A Home "Keep Watching" card (spec §5.10): poster + the same progress-bar
 * overlay used on movie cards / episode rows (`WatchProgressOverlay`, §5.9),
 * fed the item's own progress. Clicking resumes via the standard flow.
 */
export default function KeepWatchingCard({
  item,
  onActivate,
  onMenu,
}: KeepWatchingCardProps) {
  const { title, subtitle, poster } = describe(item);
  return (
    <div
      className="group relative"
      onContextMenu={(e) => {
        e.preventDefault();
        onMenu(item, e.clientX, e.clientY);
      }}
    >
      <button
        onClick={() => onActivate(item)}
        data-testid="keep-watching-card"
        title={title}
        className="block w-full text-left"
      >
        <Poster
          url={poster}
          title={title}
          overlay={<WatchProgressOverlay progress={item.progress} showCheck={false} />}
        />
        <p className="mt-2 truncate text-sm text-zinc-200 group-hover:text-white">
          {title}
        </p>
        <p className="mt-0.5 h-4 truncate text-xs text-zinc-500">{subtitle}</p>
      </button>
      <button
        type="button"
        aria-label="More options"
        data-testid="keep-watching-menu-button"
        onClick={(e) => {
          e.stopPropagation();
          const r = e.currentTarget.getBoundingClientRect();
          onMenu(item, r.right, r.bottom);
        }}
        className="absolute right-1.5 top-1.5 rounded-md bg-black/60 px-1.5 py-0.5 text-sm leading-none text-zinc-100 opacity-0 backdrop-blur transition hover:bg-black/80 focus-visible:opacity-100 group-hover:opacity-100"
      >
        ⋯
      </button>
    </div>
  );
}
