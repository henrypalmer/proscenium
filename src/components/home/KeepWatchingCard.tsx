import { Poster } from "../vod/PosterGrid";
import WatchProgressOverlay from "../vod/WatchProgressOverlay";
import type { ContinueWatchingItem } from "../../types";

interface KeepWatchingCardProps {
  item: ContinueWatchingItem;
  onActivate: (item: ContinueWatchingItem) => void;
}

/** Title / subtitle / poster for a Keep Watching entry. Episodes show their
 * series (with the episode label) and fall back to the series poster. */
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
    poster: episode.posterUrl ?? series?.posterUrl ?? null,
  };
}

/**
 * A Home "Keep Watching" card (spec §5.10): poster + the same progress-bar
 * overlay used on movie cards / episode rows (`WatchProgressOverlay`, §5.9),
 * fed the item's own progress. Clicking resumes via the standard flow.
 */
export default function KeepWatchingCard({ item, onActivate }: KeepWatchingCardProps) {
  const { title, subtitle, poster } = describe(item);
  return (
    <button
      onClick={() => onActivate(item)}
      data-testid="keep-watching-card"
      title={title}
      className="group block w-full text-left"
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
  );
}
