import { useState } from "react";
import Placeholder from "../common/Placeholder";
import { displayChannelName } from "../../lib/utils";
import type { LiveChannel } from "../../types";

interface ChannelCardProps {
  channel: LiveChannel;
  /** Show the category chip (used in the "All Channels" view, spec §5.3). */
  showCategory: boolean;
  onActivate: (channel: LiveChannel) => void;
  onContextMenu: (channel: LiveChannel, x: number, y: number) => void;
  /** Compact density (Milestone 24): shorter rows, smaller logo. */
  compact?: boolean;
}

function ChannelLogo({
  url,
  name,
  compact,
}: {
  url: string | null;
  name: string;
  compact?: boolean;
}) {
  const [state, setState] = useState<"loading" | "loaded" | "error">(
    url ? "loading" : "error",
  );
  return (
    <div
      className={`relative shrink-0 overflow-hidden rounded-md bg-zinc-800 ${
        compact ? "h-8 w-8" : "h-10 w-10"
      }`}
    >
      <Placeholder label={name} />
      {url && state !== "error" && (
        <img
          src={url}
          alt=""
          loading="lazy"
          decoding="async"
          onLoad={() => setState("loaded")}
          onError={() => setState("error")}
          className={`absolute inset-0 h-full w-full object-contain transition-opacity duration-150 ${
            state === "loaded" ? "opacity-100" : "opacity-0"
          }`}
        />
      )}
    </div>
  );
}

export default function ChannelCard({
  channel,
  showCategory,
  onActivate,
  onContextMenu,
  compact = false,
}: ChannelCardProps) {
  const name = displayChannelName(channel.name);
  return (
    <button
      onClick={() => onActivate(channel)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(channel, e.clientX, e.clientY);
      }}
      data-testid="channel-card"
      className={`flex w-full items-center gap-3 border-b border-zinc-900 px-4 text-left transition-colors hover:bg-zinc-900 active:bg-zinc-800 ${
        compact ? "h-11" : "h-14"
      }`}
    >
      <ChannelLogo url={channel.logoUrl} name={name} compact={compact} />
      <span
        className={`min-w-0 flex-1 truncate text-sm ${
          channel.name.trim() ? "text-zinc-200" : "italic text-zinc-500"
        }`}
      >
        {name}
      </span>
      {showCategory && (
        <span className="max-w-40 shrink-0 truncate rounded bg-zinc-800/80 px-2 py-0.5 text-[11px] text-zinc-400">
          {channel.categoryName}
        </span>
      )}
    </button>
  );
}
