import { useEffect, useState } from "react";
import * as api from "../../lib/tauri";
import { displayChannelName } from "../../lib/utils";
import { usePlayerStore } from "../../store/playerStore";
import CachedImage from "../common/CachedImage";
import Placeholder from "../common/Placeholder";
import ScrollRow from "../common/ScrollRow";
import type { LiveChannel } from "../../types";

interface RecentChannelsRowProps {
  providerId: string;
  /** Bumped to force a re-fetch (e.g. on catalog refresh). */
  refreshKey: number;
  onActivate: (channel: LiveChannel) => void;
}

/**
 * "Recently watched" channels strip on the Live TV landing (spec §13,
 * Milestone 29): the provider's most-recently-played channels as compact,
 * clickable chips. Local-only; re-fetches whenever the player closes so a
 * channel just watched appears immediately. Omitted when there is no history.
 */
export default function RecentChannelsRow({
  providerId,
  refreshKey,
  onActivate,
}: RecentChannelsRowProps) {
  const [items, setItems] = useState<LiveChannel[]>([]);
  // Re-fetch after a watch session ends (open → closed) so the row stays current.
  const playerOpen = usePlayerStore((s) => s.open);

  useEffect(() => {
    let cancelled = false;
    void api.getRecentChannels(providerId, 15).then(
      (channels) => {
        if (!cancelled) setItems(channels);
      },
      () => {
        if (!cancelled) setItems([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [providerId, refreshKey, playerOpen]);

  if (items.length === 0) return null;

  return (
    <section
      data-testid="recent-channels"
      className="border-b border-zinc-900 px-3 py-2"
    >
      <h2 className="mb-1.5 px-1 text-xs font-semibold uppercase tracking-wide text-zinc-500">
        Recently watched
      </h2>
      <ScrollRow>
        {items.map((channel) => {
          const name = displayChannelName(channel.name);
          return (
            <button
              key={channel.id}
              onClick={() => onActivate(channel)}
              title={name}
              data-testid="recent-channel"
              className="group flex w-[112px] shrink-0 flex-col items-center gap-1.5 rounded-lg p-2 text-center transition-transform hover:scale-[1.04] active:scale-[0.98] hover:bg-zinc-900 motion-reduce:transition-none motion-reduce:hover:scale-100"
            >
              <div className="relative h-12 w-12 overflow-hidden rounded-md bg-zinc-800">
                <Placeholder label={name} />
                <CachedImage
                  url={channel.logoUrl}
                  className="absolute inset-0 h-full w-full object-contain"
                />
              </div>
              <span className="w-full truncate text-xs text-zinc-300">
                {name}
              </span>
            </button>
          );
        })}
      </ScrollRow>
    </section>
  );
}
