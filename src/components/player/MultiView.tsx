import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import * as api from "../../lib/tauri";
import { inTauri } from "../../lib/tauri";
import { useWindowKeydown } from "../../lib/keyboard";
import { MAX_TILES, useMultiViewStore } from "../../store/multiViewStore";
import type { MvTile } from "../../store/multiViewStore";
import ChannelList from "../live/ChannelList";
import { addSlot, computeLayout, type LayoutRect } from "./multiViewLayout";

/**
 * Multi-view overlay (spec §13 / Milestone 37). Renders a transparent grid of
 * cells over the compositor's output: video shows through each cell, the gaps
 * show the compositor backdrop, and chrome is drawn on top. Each cell's screen
 * rectangle is reported to the backend so the compositor knows where to draw
 * that tile; this is the single source of layout, recomputed on resize.
 */
export default function MultiView() {
  const active = useMultiViewStore((s) => s.active);
  const tiles = useMultiViewStore((s) => s.tiles);
  const layout = useMultiViewStore((s) => s.layout);
  const focusId = useMultiViewStore((s) => s.focusId);
  const activeAudio = useMultiViewStore((s) => s.activeAudio);
  const pickerOpen = useMultiViewStore((s) => s.pickerOpen);
  const error = useMultiViewStore((s) => s.error);

  const exit = useMultiViewStore((s) => s.exit);
  const setLayout = useMultiViewStore((s) => s.setLayout);
  const setActiveAudio = useMultiViewStore((s) => s.setActiveAudio);
  const setVolume = useMultiViewStore((s) => s.setVolume);
  const removeTile = useMultiViewStore((s) => s.removeTile);
  const promote = useMultiViewStore((s) => s.promote);
  const requestPicker = useMultiViewStore((s) => s.requestPicker);
  const openPicker = useMultiViewStore((s) => s.openPicker);
  const closePicker = useMultiViewStore((s) => s.closePicker);
  const addChannel = useMultiViewStore((s) => s.addChannel);

  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });

  // Track the container (= player area) size in CSS px; resize drives relayout.
  useLayoutEffect(() => {
    if (!active) return;
    const measure = () => {
      const el = containerRef.current;
      if (el) setSize({ w: el.clientWidth, h: el.clientHeight });
    };
    measure();
    const ro = new ResizeObserver(measure);
    if (containerRef.current) ro.observe(containerRef.current);
    return () => ro.disconnect();
  }, [active]);

  const focusIndex = useMemo(
    () => Math.max(0, tiles.findIndex((t) => t.id === focusId)),
    [tiles, focusId],
  );
  const rects = useMemo(
    () => computeLayout(tiles.length, layout, focusIndex, size.w, size.h),
    [tiles.length, layout, focusIndex, size],
  );
  const add = addSlot(tiles.length, layout, size.w, size.h);

  // Report each tile's physical-pixel rect to the compositor (CSS px × DPR).
  useEffect(() => {
    if (!active || size.w === 0 || rects.length !== tiles.length) return;
    const dpr = window.devicePixelRatio || 1;
    const payload = tiles.map((t, i) => ({
      tileId: t.id,
      x: Math.round(rects[i].x * dpr),
      y: Math.round(rects[i].y * dpr),
      w: Math.round(rects[i].w * dpr),
      h: Math.round(rects[i].h * dpr),
    }));
    void api.mv.setRects(payload);
  }, [active, rects, tiles, size]);

  useWindowKeydown(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (useMultiViewStore.getState().pickerOpen) closePicker();
        else void exit();
      }
    },
    [exit, closePicker],
    { enabled: active, ignoreEditable: true },
  );

  if (!active) return null;

  const canAdd = tiles.length < MAX_TILES;
  const activeTile = tiles.find((t) => t.id === activeAudio);

  return (
    <div
      ref={containerRef}
      data-testid="multiview"
      className="pointer-events-none fixed inset-0 z-50"
    >
      {tiles.map((t, i) =>
        rects[i] ? (
          <Tile
            key={t.id}
            tile={t}
            rect={rects[i]}
            isAudio={t.id === activeAudio}
            isFocus={layout === "focus" && t.id === focusId}
            canClose={t.id !== 0}
            onActivate={() => {
              // In Focus, clicking a tile promotes it to the main pane and
              // moves audio there in one go; in Grid it just claims audio.
              void setActiveAudio(t.id);
              if (layout === "focus") promote(t.id);
            }}
            onClaimAudio={() => void setActiveAudio(t.id)}
            onPromote={() => promote(t.id)}
            onClose={() => void removeTile(t.id)}
          />
        ) : null,
      )}

      {add && canAdd && <AddCell rect={add} onClick={openPicker} />}

      {/* Floating control bar. */}
      <div className="pointer-events-auto absolute bottom-4 left-1/2 flex -translate-x-1/2 items-center gap-1 rounded-full border border-white/10 bg-black/80 px-2 py-1.5 text-sm text-zinc-200 shadow-xl backdrop-blur">
        <button
          onClick={() => void exit()}
          title="Exit multi-view (Esc)"
          className="rounded-full px-3 py-1 text-zinc-300 hover:bg-white/10 hover:text-white"
        >
          Exit
        </button>
        <span className="mx-1 h-5 w-px bg-white/15" />
        <div className="flex items-center rounded-full bg-white/5 p-0.5">
          <button
            onClick={() => setLayout("grid")}
            className={`rounded-full px-3 py-1 text-xs ${layout === "grid" ? "bg-white/15 text-white" : "text-zinc-400 hover:text-white"}`}
          >
            Grid
          </button>
          <button
            onClick={() => setLayout("focus")}
            className={`rounded-full px-3 py-1 text-xs ${layout === "focus" ? "bg-white/15 text-white" : "text-zinc-400 hover:text-white"}`}
          >
            Focus
          </button>
        </div>
        <span className="mx-1 h-5 w-px bg-white/15" />
        {/* Volume routes to the active-audio tile. */}
        <div className="flex items-center gap-2 px-1" title="Volume (active tile)">
          <svg viewBox="0 0 24 24" fill="currentColor" className="h-4 w-4 text-zinc-400">
            <path d="M3 10v4h4l5 5V5L7 10H3Zm13.5 2a4.5 4.5 0 0 0-2.5-4v8a4.5 4.5 0 0 0 2.5-4Z" />
          </svg>
          <input
            type="range"
            min={0}
            max={100}
            step={1}
            value={Math.round(activeTile?.state?.volume ?? 100)}
            onChange={(e) => void setVolume(Number(e.target.value))}
            aria-label="Volume"
            className="h-1 w-24 cursor-pointer accent-emerald-400"
          />
        </div>
        <span className="mx-1 h-5 w-px bg-white/15" />
        <button
          onClick={requestPicker}
          title="Add a channel"
          className="rounded-full bg-emerald-600 px-3 py-1 text-xs font-medium text-white hover:bg-emerald-500"
        >
          + Add channel
        </button>
      </div>

      {error && (
        <div className="pointer-events-none absolute bottom-20 left-1/2 -translate-x-1/2 rounded-md bg-red-950/90 px-3 py-1.5 text-xs text-red-200 shadow">
          {error}
        </div>
      )}

      {pickerOpen && tiles[0] && (
        <ChannelPicker
          providerId={tiles[0].providerId}
          onPick={(c) => void addChannel(c)}
          onClose={closePicker}
        />
      )}
    </div>
  );
}

interface TileProps {
  tile: MvTile;
  rect: LayoutRect;
  isAudio: boolean;
  isFocus: boolean;
  canClose: boolean;
  /** Body click: claim audio (Grid) or promote + claim audio (Focus). */
  onActivate: () => void;
  /** Speaker button: claim audio only (without promoting). */
  onClaimAudio: () => void;
  onPromote: () => void;
  onClose: () => void;
}

function Tile({
  tile,
  rect,
  isAudio,
  isFocus,
  canClose,
  onActivate,
  onClaimAudio,
  onPromote,
  onClose,
}: TileProps) {
  const buffering = tile.state ? tile.state.buffering : true;
  const fatal = tile.error ?? tile.state?.error ?? null;
  // The cell is transparent (video shows through); a ring marks audio focus.
  return (
    <div
      data-testid="multiview-tile"
      role="button"
      tabIndex={0}
      aria-label={`${tile.title}${isAudio ? " — has audio" : ""}${isFocus ? " — primary" : ""}`}
      onClick={onActivate}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onActivate();
        }
      }}
      style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
      className={`group pointer-events-auto absolute overflow-hidden rounded-md ring-inset focus:outline-none focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-400 ${
        isAudio ? "ring-2 ring-emerald-400" : "ring-1 ring-white/10"
      }`}
    >
      {/* Dev mock (no native video): show a placeholder so the grid is visible. */}
      {!inTauri && (
        <div className="absolute inset-0 -z-10 flex items-center justify-center bg-gradient-to-br from-zinc-800 via-zinc-900 to-black">
          <span className="select-none text-xs font-semibold tracking-widest text-zinc-600">
            {tile.title}
          </span>
        </div>
      )}

      {/* Buffering / error indicator for this tile. */}
      {fatal ? (
        <div className="absolute inset-0 flex items-center justify-center bg-zinc-950/80 px-3 text-center text-xs text-red-300">
          {fatal}
        </div>
      ) : buffering ? (
        <div className="absolute left-2 top-2 rounded bg-black/60 px-1.5 py-0.5 text-[10px] text-zinc-300">
          Loading…
        </div>
      ) : null}

      {/* Top chrome on hover/focus: label + actions. */}
      <div className="pointer-events-none absolute inset-x-0 top-0 flex items-center gap-2 bg-gradient-to-b from-black/70 to-transparent px-2 py-1.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 motion-reduce:transition-none">
        <span className="min-w-0 flex-1 truncate text-xs font-medium text-white">
          {tile.title}
        </span>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onClaimAudio();
          }}
          title={isAudio ? "Has audio" : "Move audio here"}
          className={`pointer-events-auto rounded p-1 ${isAudio ? "text-emerald-400" : "text-zinc-300 hover:bg-white/15 hover:text-white"}`}
        >
          <svg viewBox="0 0 24 24" fill="currentColor" className="h-4 w-4">
            {isAudio ? (
              <path d="M3 10v4h4l5 5V5L7 10H3Zm13.5 2a4.5 4.5 0 0 0-2.5-4v8a4.5 4.5 0 0 0 2.5-4Z" />
            ) : (
              <path d="M5 10v4h3l4 4V6L8 10H5Zm10 .5v3a3 3 0 0 0 0-3Z" />
            )}
          </svg>
        </button>
        {!isFocus && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onPromote();
            }}
            title="Make primary (Focus layout)"
            className="pointer-events-auto rounded p-1 text-zinc-300 hover:bg-white/15 hover:text-white"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-4 w-4">
              <path d="M4 14v6h6M20 10V4h-6M14 10l6-6M10 14l-6 6" />
            </svg>
          </button>
        )}
        {canClose && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onClose();
            }}
            title="Close tile"
            className="pointer-events-auto rounded p-1 text-zinc-300 hover:bg-white/15 hover:text-white"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-4 w-4">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}

function AddCell({ rect, onClick }: { rect: LayoutRect; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
      className="pointer-events-auto absolute flex flex-col items-center justify-center gap-1 rounded-md border border-dashed border-white/15 bg-white/5 text-zinc-400 hover:bg-white/10 hover:text-white"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" className="h-7 w-7">
        <path d="M12 5v14M5 12h14" />
      </svg>
      <span className="text-xs">Add channel</span>
    </button>
  );
}

function ChannelPicker({
  providerId,
  onPick,
  onClose,
}: {
  providerId: string;
  onPick: (c: { providerId: string; contentId: string; title: string; logoUrl?: string | null }) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  return (
    <div className="pointer-events-auto absolute inset-0 z-10 flex items-center justify-center bg-black/60 p-6">
      <div className="flex h-[80%] w-full max-w-2xl flex-col overflow-hidden rounded-xl border border-zinc-800 bg-zinc-950 shadow-2xl">
        <div className="flex items-center gap-3 border-b border-zinc-800 px-4 py-3">
          <h2 className="text-sm font-semibold text-white">Add a channel</h2>
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter channels…"
            className="ml-auto w-56 rounded-md border border-zinc-700 bg-zinc-900 px-2.5 py-1.5 text-sm text-zinc-100 placeholder:text-zinc-500 focus:border-zinc-500 focus:outline-none"
          />
          <button
            onClick={onClose}
            aria-label="Close"
            className="rounded p-1 text-zinc-400 hover:bg-white/10 hover:text-white"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-5 w-5">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </div>
        <div className="min-h-0 flex-1">
          <ChannelList
            providerId={providerId}
            categoryId={null}
            showCategory
            version={0}
            query={query}
            onActivate={(ch) =>
              onPick({
                providerId,
                contentId: ch.id,
                title: ch.name,
                logoUrl: ch.logoUrl,
              })
            }
            onContextMenu={() => undefined}
          />
        </div>
      </div>
    </div>
  );
}
