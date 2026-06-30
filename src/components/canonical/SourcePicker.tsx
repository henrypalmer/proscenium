import { useState } from "react";
import * as api from "../../lib/tauri";
import { usePlayerStore } from "../../store/playerStore";
import type { StreamCandidate } from "../../types";

type Phase =
  | { phase: "idle" }
  | { phase: "searching" }
  | { phase: "results"; sources: StreamCandidate[] }
  | { phase: "error"; message: string };

interface Props {
  kind: "movie" | "series";
  imdbId: string;
  /** Title used for the player's now-playing label. */
  title: string;
  /** Series episode addressing (M40 slice 4); omitted for movies. */
  season?: number;
  episode?: number;
}

/**
 * Source picker (M40 slice 3): on click, resolve playback sources for a canonical
 * title across the enabled providers, then let the user pick which to stream.
 * Selecting a provider source plays it through the existing player path.
 * **"No sources found"** is a first-class state.
 */
export default function SourcePicker({ kind, imdbId, title, season, episode }: Props) {
  const [state, setState] = useState<Phase>({ phase: "idle" });

  const search = async () => {
    setState({ phase: "searching" });
    try {
      const sources = await api.resolveSources(kind, imdbId, season, episode);
      setState({ phase: "results", sources });
    } catch (e) {
      setState({ phase: "error", message: String(e) });
    }
  };

  const play = async (c: StreamCandidate) => {
    if (!c.providerId || !c.contentId) return; // direct-URL (addon) sources: M41
    const args = {
      providerId: c.providerId,
      contentType: c.contentType,
      contentId: c.contentId,
      title,
    };
    // Resume from the title's saved position across *any* source (M40 slice 5);
    // an un-matched title returns null and the player resumes per-item as usual.
    const prog = await api
      .getCanonicalProgress(kind, imdbId, season, episode)
      .catch(() => null);
    if (prog && !prog.completed && prog.positionSeconds >= 5) {
      void usePlayerStore.getState().playDirect(args, prog.positionSeconds);
    } else {
      void usePlayerStore.getState().openContent(args);
    }
  };

  if (state.phase === "idle") {
    return (
      <button
        onClick={() => void search()}
        data-testid="resolve-sources"
        className="rounded-md bg-zinc-100 px-5 py-2 text-sm font-semibold text-zinc-900 hover:bg-white"
      >
        ▶ Play
      </button>
    );
  }

  if (state.phase === "searching") {
    return (
      <div data-testid="sources-searching" className="text-sm text-zinc-400">
        Searching sources across your providers…
      </div>
    );
  }

  if (state.phase === "error") {
    return (
      <div className="text-sm text-red-400">
        Couldn’t search sources: {state.message}{" "}
        <button onClick={() => void search()} className="underline hover:text-red-300">
          Retry
        </button>
      </div>
    );
  }

  if (state.sources.length === 0) {
    return (
      <div
        data-testid="no-sources"
        className="max-w-md rounded-md border border-zinc-700 bg-zinc-900/60 px-4 py-3 text-sm text-zinc-400"
      >
        No sources found across your enabled providers.{" "}
        <button onClick={() => void search()} className="underline hover:text-zinc-200">
          Try again
        </button>
      </div>
    );
  }

  return (
    <div data-testid="source-list" className="w-full max-w-md space-y-1.5">
      <p className="text-xs font-medium uppercase tracking-wide text-zinc-500">
        Choose a source
      </p>
      {state.sources.map((c, i) => (
        <button
          key={`${c.providerId ?? c.url}:${c.contentId}:${i}`}
          onClick={() => void play(c)}
          data-testid="source-option"
          className="flex w-full items-center justify-between rounded-md border border-zinc-700 bg-zinc-900/60 px-3 py-2 text-left text-sm text-zinc-200 hover:bg-zinc-800"
        >
          <span className="truncate">{c.source}</span>
          <span className="ml-3 flex shrink-0 items-center gap-2 text-xs text-zinc-400">
            {c.quality && (
              <span className="rounded bg-zinc-800 px-1.5 py-0.5">{c.quality}</span>
            )}
            {c.container && <span className="uppercase">{c.container}</span>}
          </span>
        </button>
      ))}
    </div>
  );
}
