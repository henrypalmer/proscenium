import { formatDuration } from "../../lib/utils";
import type { Episode } from "../../types";

interface EpisodeListProps {
  episodes: Episode[];
  onPlay: (episode: Episode) => void;
  onOpenExternal: (episode: Episode) => void;
}

/**
 * Episodes for the selected season (spec §5.4): number, title, duration,
 * with per-episode play and external player launch (Milestone 5 scope).
 */
export default function EpisodeList({
  episodes,
  onPlay,
  onOpenExternal,
}: EpisodeListProps) {
  return (
    <ul data-testid="episode-list" className="divide-y divide-zinc-900">
      {episodes.map((episode) => (
        <li
          key={episode.id}
          data-testid="episode-row"
          className="group flex items-center gap-3 px-2 py-2.5"
        >
          <span className="w-8 shrink-0 text-right text-sm tabular-nums text-zinc-500">
            {episode.episode}
          </span>
          <span className="min-w-0 flex-1 truncate text-sm text-zinc-200">
            {episode.title}
          </span>
          {episode.durationSeconds !== null && (
            <span className="shrink-0 text-xs text-zinc-500">
              {formatDuration(episode.durationSeconds)}
            </span>
          )}
          <button
            onClick={() => onPlay(episode)}
            data-testid="episode-play"
            title="Play"
            className="shrink-0 rounded-md bg-zinc-100 px-3 py-1 text-xs font-semibold text-zinc-900 hover:bg-white"
          >
            ▶ Play
          </button>
          <button
            onClick={() => onOpenExternal(episode)}
            data-testid="episode-external"
            title="Open in External Player"
            className="shrink-0 rounded-md border border-zinc-700 px-3 py-1 text-xs text-zinc-300 hover:bg-zinc-900"
          >
            External
          </button>
        </li>
      ))}
    </ul>
  );
}
