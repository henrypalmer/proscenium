import { useEffect, useState } from "react";
import { flushSync } from "react-dom";
import * as api from "../../lib/tauri";
import { startViewTransition } from "../../lib/viewTransition";
import CanonicalDetail from "./CanonicalDetail";
import CanonicalGrid from "./CanonicalGrid";
import ProviderBrowse from "./ProviderBrowse";
import type { CanonicalItem } from "../../types";

interface Props {
  kind: "movie" | "series";
  allLabel: string;
  emptyNoun: string;
}

/**
 * Canonical (Cinemeta) browse (M40 slice 1): a genre sidebar + paged poster grid
 * + the detail overlay. Shared by the Movies and TV Shows pages. The catalog is
 * external metadata, so it renders regardless of which providers are enabled —
 * resolving a title to a playable source (across providers) comes in slice 3.
 */
export default function CanonicalBrowse({ kind, allLabel, emptyNoun }: Props) {
  const [genres, setGenres] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<CanonicalItem | null>(null);
  const [morphId, setMorphId] = useState<string | null>(null);
  // "discover" = canonical (default); "providers" = the pre-M40 provider grid,
  // kept for un-matchable VOD (spec §19 M40 AC5).
  const [mode, setMode] = useState<"discover" | "providers">("discover");

  useEffect(() => {
    let cancelled = false;
    void api.getCanonicalGenres(kind).then(
      (g) => {
        if (!cancelled) setGenres(g);
      },
      () => {
        if (!cancelled) setGenres([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [kind]);

  const open = (item: CanonicalItem) => {
    flushSync(() => setMorphId(item.imdbId));
    startViewTransition(() => setDetail(item));
  };
  const close = () => startViewTransition(() => setDetail(null));

  const tabClass = (active: boolean) =>
    `block w-full truncate rounded-md px-3 py-1.5 text-left text-sm ${
      active
        ? "bg-zinc-800 text-white"
        : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200"
    }`;

  const modeTab = (active: boolean) =>
    `rounded-md px-3 py-1 text-xs font-medium ${
      active ? "bg-zinc-800 text-white" : "text-zinc-400 hover:text-zinc-200"
    }`;

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-1 border-b border-zinc-800/70 px-3 py-1.5">
        <button onClick={() => setMode("discover")} className={modeTab(mode === "discover")}>
          Discover
        </button>
        <button
          onClick={() => setMode("providers")}
          data-testid="provider-browse-tab"
          className={modeTab(mode === "providers")}
        >
          My Providers
        </button>
      </div>
      <div className="min-h-0 flex-1">
        {mode === "providers" ? (
          <ProviderBrowse kind={kind} />
        ) : (
          <div className="relative flex h-full">
            <nav className="w-48 shrink-0 overflow-y-auto border-r border-zinc-800/70 p-2">
              <button
                onClick={() => setSelected(null)}
                className={`mb-1 ${tabClass(selected === null)}`}
              >
                {allLabel}
              </button>
              {genres.map((g) => (
                <button key={g} onClick={() => setSelected(g)} className={tabClass(selected === g)}>
                  {g}
                </button>
              ))}
            </nav>
            <div className="min-w-0 flex-1">
              <CanonicalGrid
                kind={kind}
                genre={selected}
                onActivate={open}
                morphId={detail ? null : morphId}
                emptyNoun={emptyNoun}
              />
            </div>
            {detail && <CanonicalDetail item={detail} onClose={close} />}
          </div>
        )}
      </div>
    </div>
  );
}
