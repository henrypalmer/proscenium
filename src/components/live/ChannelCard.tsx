import { useState } from "react";
import Placeholder from "../common/Placeholder";
import type { LiveChannel } from "../../types";

interface ChannelCardProps {
  channel: LiveChannel;
  /** Show the category chip (used in the "All Channels" view, spec §5.3). */
  showCategory: boolean;
  onActivate: (channel: LiveChannel) => void;
  onContextMenu: (channel: LiveChannel, x: number, y: number) => void;
}

function ChannelLogo({ url, name }: { url: string | null; name: string }) {
  const [state, setState] = useState<"loading" | "loaded" | "error">(
    url ? "loading" : "error",
  );
  return (
    <div className="relative h-10 w-10 shrink-0 overflow-hidden rounded-md bg-zinc-800">
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
}: ChannelCardProps) {
  return (
    <button
      onClick={() => onActivate(channel)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(channel, e.clientX, e.clientY);
      }}
      data-testid="channel-card"
      className="flex h-14 w-full items-center gap-3 border-b border-zinc-900 px-4 text-left transition-colors hover:bg-zinc-900"
    >
      <ChannelLogo url={channel.logoUrl} name={channel.name} />
      <span className="min-w-0 flex-1 truncate text-sm text-zinc-200">
        {channel.name}
      </span>
      {showCategory && (
        <span className="max-w-40 shrink-0 truncate rounded bg-zinc-800/80 px-2 py-0.5 text-[11px] text-zinc-400">
          {channel.categoryName}
        </span>
      )}
    </button>
  );
}
