import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useRef } from "react";
import { usePagedLiveChannels } from "../../hooks/useCatalog";
import { useCatalogStore } from "../../store/catalogStore";
import { useDensity } from "../../lib/useDensity";
import SkeletonCard from "../common/SkeletonCard";
import ChannelCard from "./ChannelCard";
import type { LiveChannel } from "../../types";

// Row heights per density (Milestone 24) — must match ChannelCard/SkeletonCard.
const ROW_HEIGHT_COMFORTABLE = 56; // h-14
const ROW_HEIGHT_COMPACT = 44; // h-11
const INITIAL_SKELETON_ROWS = 14;

interface ChannelListProps {
  providerIds: string[];
  categoryId: string | null;
  showCategory: boolean;
  version: number;
  /** In-section name filter (spec §5.3); empty string = no filter. */
  query: string;
  onActivate: (channel: LiveChannel) => void;
  onContextMenu: (channel: LiveChannel, x: number, y: number) => void;
}

/**
 * Virtualized channel list (spec §10): only rows in or near the viewport
 * exist in the DOM, with pages fetched on demand as the user scrolls.
 */
export default function ChannelList({
  providerIds,
  categoryId,
  showCategory,
  version,
  query,
  onActivate,
  onContextMenu,
}: ChannelListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const { total, getItem, ensureRange } = usePagedLiveChannels(
    providerIds,
    categoryId,
    version,
    query,
  );
  const refreshing = useCatalogStore((s) => s.refreshing);
  const refresh = useCatalogStore((s) => s.refresh);

  const compact = useDensity() === "compact";
  const rowHeight = compact ? ROW_HEIGHT_COMPACT : ROW_HEIGHT_COMFORTABLE;

  // While the first page loads, render a fixed batch of skeleton rows.
  const count = total ?? INITIAL_SKELETON_ROWS;

  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight,
    overscan: 10,
  });
  const virtualItems = virtualizer.getVirtualItems();

  // Re-measure when the density (row height) changes.
  useEffect(() => {
    virtualizer.measure();
  }, [virtualizer, rowHeight]);

  // Fetch the pages backing the currently visible range.
  useEffect(() => {
    if (total === null || virtualItems.length === 0) return;
    ensureRange(
      virtualItems[0].index,
      virtualItems[virtualItems.length - 1].index,
    );
  }, [total, virtualItems, ensureRange]);

  // Back to the top when the category, filter, or catalog version changes.
  useEffect(() => {
    parentRef.current?.scrollTo({ top: 0 });
  }, [categoryId, version, query]);

  if (total === 0 && query !== "") {
    // Spec §5.3: nothing in the active category matches the filter text.
    return (
      <div
        data-testid="channel-filter-empty"
        className="flex h-full flex-col items-center justify-center gap-1 text-center"
      >
        <p className="text-sm font-medium text-zinc-400">No channels match</p>
        <p className="max-w-sm text-xs text-zinc-600">“{query}”</p>
      </div>
    );
  }

  if (total === 0) {
    // Spec §12: empty catalog → instructional empty state with Refresh.
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
        <p className="text-sm font-medium text-zinc-400">No channels here yet</p>
        <p className="max-w-sm text-xs text-zinc-600">
          {categoryId
            ? "This category has no channels."
            : "Refresh the catalog to pull channels from your provider."}
        </p>
        {!categoryId && (
          <button
            onClick={() => void refresh()}
            disabled={refreshing}
            className="rounded-md bg-zinc-100 px-4 py-1.5 text-xs font-semibold text-zinc-900 hover:bg-white disabled:opacity-50"
          >
            {refreshing ? "Refreshing…" : "Refresh"}
          </button>
        )}
      </div>
    );
  }

  return (
    <div
      ref={parentRef}
      data-testid="channel-scroll"
      className="h-full overflow-y-auto"
    >
      <div
        style={{ height: virtualizer.getTotalSize(), position: "relative" }}
      >
        {virtualItems.map((row) => {
          const item = total === null ? undefined : getItem(row.index);
          return (
            <div
              key={row.key}
              data-index={row.index}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                height: row.size,
                transform: `translateY(${row.start}px)`,
              }}
            >
              {item ? (
                <ChannelCard
                  channel={item}
                  showCategory={showCategory}
                  onActivate={onActivate}
                  onContextMenu={onContextMenu}
                  compact={compact}
                />
              ) : (
                <SkeletonCard compact={compact} />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
