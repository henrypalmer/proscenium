import type { ListSummary } from "../../types";

interface ListCoverCardProps {
  list: ListSummary;
  onOpen: (listId: string) => void;
  onMenu: (list: ListSummary, x: number, y: number) => void;
}

/** One tile of the 2×2 mosaic: a poster if present, else a neutral fill (no
 * "?" placeholder — that read as broken on art-less covers, Milestone 25). */
function MosaicTile({ url }: { url: string | null }) {
  if (!url) {
    return <div className="bg-zinc-900" />;
  }
  return (
    <div className="relative bg-zinc-900">
      <img
        src={url}
        alt=""
        loading="lazy"
        decoding="async"
        className="h-full w-full object-cover"
        onError={(e) => {
          (e.currentTarget as HTMLImageElement).style.visibility = "hidden";
        }}
      />
    </div>
  );
}

/** Neutral cover for an empty list (spec §1, Milestone 25) — replaces the 2×2
 * grid of "?" placeholders that looked broken/unfinished. */
function EmptyCover() {
  return (
    <div
      data-testid="empty-list-cover"
      className="flex h-full w-full flex-col items-center justify-center gap-2 text-zinc-700"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" className="h-8 w-8">
        <path d="M4 7h11M4 12h11M4 17h7" />
        <path d="M19 14v6M16 17h6" strokeLinecap="round" />
      </svg>
      <span className="text-xs">Empty list</span>
    </div>
  );
}

/**
 * A custom list rendered as a Home cover card (spec §5.10): a 2×2 poster mosaic
 * of the list's first items + name + item count. Click opens List Detail;
 * right-click / ⋯ opens the rename/delete menu.
 */
export default function ListCoverCard({ list, onOpen, onMenu }: ListCoverCardProps) {
  const tiles = [0, 1, 2, 3].map((i) => list.coverPosters[i] ?? null);
  const isEmpty = list.itemCount === 0;
  return (
    <div
      className="group relative transition-transform duration-200 ease-out hover:z-10 hover:scale-[1.04] active:scale-[0.98] motion-reduce:transition-none motion-reduce:hover:scale-100"
      onContextMenu={(e) => {
        e.preventDefault();
        onMenu(list, e.clientX, e.clientY);
      }}
    >
      <button
        onClick={() => onOpen(list.id)}
        data-testid="list-cover-card"
        title={list.name}
        className="block w-full text-left"
      >
        <div className="aspect-[2/3] overflow-hidden rounded-lg border border-zinc-800 bg-zinc-950">
          {isEmpty ? (
            <EmptyCover />
          ) : (
            <div className="grid h-full w-full grid-cols-2 grid-rows-2 gap-0.5">
              {tiles.map((url, i) => (
                <MosaicTile key={i} url={url} />
              ))}
            </div>
          )}
        </div>
        <p className="mt-2 truncate text-sm text-zinc-200 group-hover:text-white">
          {list.name}
        </p>
        <p className="mt-0.5 h-4 truncate text-xs text-zinc-500">
          {list.itemCount} {list.itemCount === 1 ? "item" : "items"}
        </p>
      </button>
      <button
        type="button"
        aria-label="List options"
        data-testid="list-cover-menu-button"
        onClick={(e) => {
          e.stopPropagation();
          const r = e.currentTarget.getBoundingClientRect();
          onMenu(list, r.right, r.bottom);
        }}
        className="absolute right-1.5 top-1.5 rounded-md bg-black/60 px-1.5 py-0.5 text-sm leading-none text-zinc-100 opacity-0 backdrop-blur transition hover:bg-black/80 focus-visible:opacity-100 group-hover:opacity-100"
      >
        ⋯
      </button>
    </div>
  );
}
