import { useEffect, useState } from "react";
import { flushSync } from "react-dom";
import { useNavigate, useParams } from "react-router-dom";
import ContextMenu from "../components/common/ContextMenu";
import Placeholder from "../components/common/Placeholder";
import ListEditorDialog from "../components/lists/ListEditorDialog";
import MovieCard from "../components/vod/MovieCard";
import MovieDetail from "../components/vod/MovieDetail";
import SeriesCard from "../components/vod/SeriesCard";
import SeriesDetail from "../components/vod/SeriesDetail";
import * as api from "../lib/tauri";
import { startViewTransition } from "../lib/viewTransition";
import { useCatalogStore } from "../store/catalogStore";
import { useListsStore } from "../store/listsStore";
import { usePlayerStore } from "../store/playerStore";
import { useProgressStore } from "../store/progressStore";
import type { LiveChannel, ListContentType, Movie, Series, UserListItem } from "../types";

/** Stable membership key for a list item. */
function itemKey(item: UserListItem): string {
  if (item.kind === "movie") return `movie:${item.movie.id}`;
  if (item.kind === "series") return `series:${item.series.id}`;
  return `live:${item.channel.id}`;
}

function refOf(item: UserListItem): { contentType: ListContentType; contentId: string } {
  if (item.kind === "movie") return { contentType: "movie", contentId: item.movie.id };
  if (item.kind === "series") return { contentType: "series", contentId: item.series.id };
  return { contentType: "live", contentId: item.channel.id };
}

/** A poster-shaped tile for a live channel inside the list grid. */
function ChannelTile({
  channel,
  onPlay,
}: {
  channel: LiveChannel;
  onPlay: () => void;
}) {
  return (
    <button
      onClick={onPlay}
      data-testid="list-channel-tile"
      title={channel.name}
      className="group block w-full text-left"
    >
      <div className="relative aspect-[2/3] overflow-hidden rounded-lg bg-zinc-900">
        <Placeholder label={channel.name} />
        {channel.logoUrl && (
          <img
            src={channel.logoUrl}
            alt=""
            loading="lazy"
            decoding="async"
            className="absolute inset-0 h-full w-full object-contain p-4"
            onError={(e) => {
              (e.currentTarget as HTMLImageElement).style.visibility = "hidden";
            }}
          />
        )}
      </div>
      <p className="mt-2 truncate text-sm text-zinc-200 group-hover:text-white">
        {channel.name}
      </p>
      <p className="mt-0.5 h-4 truncate text-xs text-zinc-500">Live TV</p>
    </button>
  );
}

export default function ListDetail() {
  const { listId } = useParams<{ listId: string }>();
  const navigate = useNavigate();
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const providerId = activeProvider?.id ?? null;

  const lists = useListsStore((s) => s.lists);
  const loadLists = useListsStore((s) => s.load);
  const renameList = useListsStore((s) => s.rename);
  const removeList = useListsStore((s) => s.remove);
  const removeItem = useListsStore((s) => s.removeItem);

  const [items, setItems] = useState<UserListItem[]>([]);
  const [renaming, setRenaming] = useState(false);
  const [menu, setMenu] = useState<{ item: UserListItem; x: number; y: number } | null>(
    null,
  );
  /** Card whose poster morphs in/out of the in-place detail overlay (kept set
   * after close so the reverse morph lands back on the same card). */
  const [morph, setMorph] = useState<{ type: "movie" | "series"; id: string } | null>(
    null,
  );
  /** Detail shown as an in-place overlay (not a route change) so the list stays
   * mounted — scroll is preserved and the poster morphs back on close. */
  const [detail, setDetail] = useState<
    { type: "movie"; item: Movie } | { type: "series"; item: Series } | null
  >(null);

  const list = lists.find((l) => l.id === listId);

  // Ensure the store has this provider's lists (for the header name/meta).
  useEffect(() => {
    if (providerId) void loadLists(providerId);
  }, [providerId, loadLists]);

  // Load the list's resolved items and the movie progress overlays.
  useEffect(() => {
    if (!listId) return;
    if (providerId) void useProgressStore.getState().loadSection(providerId, "movie");
    let cancelled = false;
    void api.getListItems(listId).then(
      (its) => {
        if (!cancelled) setItems(its);
      },
      () => {
        if (!cancelled) setItems([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [listId, providerId]);

  if (!activeProvider) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <p className="text-sm font-medium text-zinc-400">No provider selected</p>
      </div>
    );
  }
  if (!listId) return null;
  const pid = activeProvider.id;

  // Open the detail as an in-place overlay with the poster morph (same pattern
  // as the grids/Home): flush the clicked card's shared name before the
  // snapshot, then mount the overlay. The list stays mounted, so closing morphs
  // the poster back into the same card with scroll preserved.
  const openMovie = (item: Extract<UserListItem, { kind: "movie" }>) => {
    flushSync(() => setMorph({ type: "movie", id: item.movie.id }));
    startViewTransition(() => setDetail({ type: "movie", item: item.movie }));
  };
  const openSeries = (item: Extract<UserListItem, { kind: "series" }>) => {
    flushSync(() => setMorph({ type: "series", id: item.series.id }));
    startViewTransition(() => setDetail({ type: "series", item: item.series }));
  };
  const closeDetail = () => startViewTransition(() => setDetail(null));
  const playChannel = (channel: LiveChannel) =>
    void usePlayerStore.getState().openContent({
      providerId: pid,
      contentType: "live",
      contentId: channel.id,
      title: channel.name,
    });

  const remove = (item: UserListItem) => {
    const { contentType, contentId } = refOf(item);
    void removeItem(listId, contentType, contentId);
    setItems((prev) => prev.filter((it) => itemKey(it) !== itemKey(item)));
  };

  const activate = (item: UserListItem) => {
    if (item.kind === "movie") openMovie(item);
    else if (item.kind === "series") openSeries(item);
    else playChannel(item.channel);
  };

  return (
    <div className="relative h-full">
      <div className="h-full overflow-y-auto px-6 pb-10">
        <div className="mx-auto max-w-6xl">
          <div className="mb-6 flex items-center gap-3">
            <button
              onClick={() => navigate(-1)}
              aria-label="Back"
              className="rounded-md border border-zinc-700 px-2 py-1 text-sm text-zinc-300 hover:bg-zinc-800"
            >
              ←
            </button>
            <div className="min-w-0 flex-1">
              <h1 className="truncate text-xl font-semibold text-white">
                {list?.name ?? "List"}
              </h1>
              <p className="text-xs text-zinc-500">
                {items.length} {items.length === 1 ? "item" : "items"}
              </p>
            </div>
            <button
              onClick={() => setRenaming(true)}
              className="rounded-md border border-zinc-700 px-3 py-1.5 text-sm text-zinc-200 hover:bg-zinc-800"
            >
              Rename
            </button>
            <button
              onClick={() => {
                void removeList(listId);
                navigate(-1);
              }}
              data-testid="delete-list"
              className="rounded-md border border-zinc-700 px-3 py-1.5 text-sm text-rose-300 hover:bg-rose-950/40"
            >
              Delete
            </button>
          </div>

          {items.length === 0 ? (
            <div className="flex h-64 flex-col items-center justify-center gap-2 text-center">
              <p className="text-sm font-medium text-zinc-400">This list is empty</p>
              <p className="max-w-sm text-xs text-zinc-600">
                Add movies, series, or channels with the "Add to list…" option from
                any title's right-click menu.
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(140px,1fr))] gap-4">
              {items.map((item) => (
                <div
                  key={itemKey(item)}
                  className="group relative"
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setMenu({ item, x: e.clientX, y: e.clientY });
                  }}
                >
                  {item.kind === "movie" && (
                    <MovieCard
                      movie={item.movie}
                      providerId={pid}
                      onActivate={() => openMovie(item)}
                      onContextMenu={(_m, x, y) => setMenu({ item, x, y })}
                      morphActive={
                        detail === null &&
                        morph?.type === "movie" &&
                        morph.id === item.movie.id
                      }
                    />
                  )}
                  {item.kind === "series" && (
                    <SeriesCard
                      series={item.series}
                      onActivate={() => openSeries(item)}
                      morphActive={
                        detail === null &&
                        morph?.type === "series" &&
                        morph.id === item.series.id
                      }
                    />
                  )}
                  {item.kind === "live" && (
                    <ChannelTile channel={item.channel} onPlay={() => playChannel(item.channel)} />
                  )}
                  <button
                    type="button"
                    aria-label="Remove from list"
                    data-testid="list-item-remove"
                    onClick={(e) => {
                      e.stopPropagation();
                      remove(item);
                    }}
                    className="absolute right-1.5 top-1.5 rounded-md bg-black/60 px-1.5 py-0.5 text-sm leading-none text-zinc-100 opacity-0 backdrop-blur transition hover:bg-black/80 focus-visible:opacity-100 group-hover:opacity-100"
                  >
                    ✕
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Detail rendered like Movies/TV Shows: absolute within this relative
          page (z-20) so it sits below the floating nav (z-30). The list stays
          mounted, so closing morphs the poster back into its card. */}
      {detail &&
        (detail.type === "movie" ? (
          <MovieDetail providerId={pid} movie={detail.item} onClose={closeDetail} />
        ) : (
          <SeriesDetail providerId={pid} series={detail.item} onClose={closeDetail} />
        ))}

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { label: "Open", onSelect: () => activate(menu.item) },
            { label: "Remove from list", onSelect: () => remove(menu.item) },
          ]}
        />
      )}

      {renaming && (
        <ListEditorDialog
          title="Rename list"
          initialName={list?.name ?? ""}
          submitLabel="Save"
          onSubmit={(name) => {
            setRenaming(false);
            void renameList(listId, name);
          }}
          onClose={() => setRenaming(false)}
        />
      )}
    </div>
  );
}
