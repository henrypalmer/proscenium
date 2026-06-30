import { useEffect, useState } from "react";
import * as api from "../../lib/tauri";
import { useCatalogStore } from "../../store/catalogStore";
import type { Series } from "../../types";

interface Props {
  imdbId: string;
  /** Canonical title — seeds the provider search. */
  name: string;
}

/**
 * Manual match override (M40 slice 4). Series carry no tmdb backstop, so the
 * auto-match (name+year) can be wrong. This lets the user search the enabled
 * providers' series catalogs and pick the correct title; the choice is persisted
 * (method "manual") and used by source resolution thereafter.
 */
export default function ManualMatch({ imdbId, name }: Props) {
  const providerIds = useCatalogStore((s) => s.providerIds);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState(name);
  const [results, setResults] = useState<Series[]>([]);
  const [savedTo, setSavedTo] = useState<string | null>(null);

  useEffect(() => {
    if (!open || providerIds.length === 0 || query.trim() === "") {
      setResults([]);
      return;
    }
    let cancelled = false;
    const t = setTimeout(() => {
      void api.search(providerIds, query, "series", undefined, 20).then(
        (r) => {
          if (!cancelled) setResults(r.series);
        },
        () => {
          if (!cancelled) setResults([]);
        },
      );
    }, 200);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [open, query, providerIds]);

  const pick = async (s: Series) => {
    await api.setManualMatch(s.providerId, "series", s.id, imdbId);
    setSavedTo(s.name);
    setOpen(false);
  };

  if (!open) {
    return (
      <div className="text-xs text-zinc-500">
        {savedTo
          ? `Matched to “${savedTo}”. `
          : "Episodes resolve to a provider series. "}
        <button
          onClick={() => setOpen(true)}
          data-testid="manual-match-open"
          className="text-zinc-300 underline hover:text-white"
        >
          {savedTo ? "Change match" : "Wrong match?"}
        </button>
      </div>
    );
  }

  return (
    <div className="w-full max-w-md rounded-md border border-zinc-700 bg-zinc-900/60 p-3">
      <p className="mb-2 text-xs font-medium uppercase tracking-wide text-zinc-500">
        Pick the correct series
      </p>
      <input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search your providers…"
        data-testid="manual-match-input"
        className="mb-2 w-full rounded bg-zinc-800 px-2 py-1.5 text-sm text-zinc-100 outline-none placeholder:text-zinc-600"
      />
      <div className="max-h-56 space-y-1 overflow-y-auto">
        {results.length === 0 ? (
          <p className="px-1 py-2 text-xs text-zinc-600">
            No matches in your enabled providers.
          </p>
        ) : (
          results.map((s) => (
            <button
              key={`${s.providerId}:${s.id}`}
              onClick={() => void pick(s)}
              data-testid="manual-match-result"
              className="flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-sm text-zinc-200 hover:bg-zinc-800"
            >
              <span className="truncate">{s.name}</span>
              <span className="ml-2 shrink-0 text-xs text-zinc-500">
                {s.releaseYear ?? ""}
              </span>
            </button>
          ))
        )}
      </div>
      <button
        onClick={() => setOpen(false)}
        className="mt-2 text-xs text-zinc-500 hover:text-zinc-300"
      >
        Cancel
      </button>
    </div>
  );
}
