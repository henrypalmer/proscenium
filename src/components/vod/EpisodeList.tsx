import { formatDuration } from "../../lib/utils";
import { useWatchProgress } from "../../store/progressStore";
import WatchProgressOverlay from "./WatchProgressOverlay";
import type { Episode } from "../../types";

interface EpisodeListProps {
  providerId: string;
  episodes: Episode[];
  onPlay: (episode: Episode) => void;
  onOpenExternal: (episode: Episode) => void;
}

/**
 * Episodes for the selected season (spec §5.4): number, title, duration, with
 * per-episode play, external launch, and watch-progress indicators (§5.9).
 */
export default function EpisodeList({
  providerId,
  episodes,
  onPlay,
  onOpenExternal,
}: EpisodeListProps) {
  return (
    <ul data-testid="episode-list" className="divide-y divide-zinc-900">
      {episodes.map((episode) => (
        <EpisodeRow
          key={episode.id}
          providerId={providerId}
          episode={episode}
          onPlay={onPlay}
          onOpenExternal={onOpenExternal}
        />
      ))}
    </ul>
  );
}

function EpisodeRow({
  providerId,
  episode,
  onPlay,
  onOpenExternal,
}: {
  providerId: string;
  episode: Episode;
  onPlay: (episode: Episode) => void;
  onOpenExternal: (episode: Episode) => void;
}) {
  const progress = useWatchProgress(providerId, "episode", episode.id);
  return (
    <li
      data-testid="episode-row"
      className="group relative flex items-center gap-3 px-2 py-2.5"
    >
      <span className="w-8 shrink-0 text-right text-sm tabular-nums text-zinc-500">
        {episode.episode}
      </span>
      <span className="flex min-w-0 flex-1 items-center gap-2">
        {progress?.completed && (
          <span
            data-testid="watched-check"
            title="Watched"
            className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-emerald-500 text-[10px] font-bold text-white"
          >
            ✓
          </span>
        )}
        <span className="truncate text-sm text-zinc-200">{episode.title}</span>
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
      {/* Bottom progress bar (in-progress only); checkmark handled inline. */}
      <WatchProgressOverlay progress={progress} showCheck={false} />
    </li>
  );
}
