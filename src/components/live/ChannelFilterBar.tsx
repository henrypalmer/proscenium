import { useEffect, useState } from "react";

/** Light debounce so a 12k-channel filter query doesn't fire per keystroke. */
const DEBOUNCE_MS = 200;

interface ChannelFilterBarProps {
  /** Called with the trimmed filter text after the debounce delay. */
  onQueryChange: (query: string) => void;
}

/**
 * In-section channel filter (spec §5.3): a name filter pinned above the
 * channel list. Filters as the user types, scoped to the active category by
 * the backend query. The input value updates immediately; the committed
 * filter text is debounced. Remounted (via key) on provider change to reset.
 */
export default function ChannelFilterBar({ onQueryChange }: ChannelFilterBarProps) {
  const [text, setText] = useState("");

  useEffect(() => {
    const timer = window.setTimeout(() => onQueryChange(text.trim()), DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [text, onQueryChange]);

  return (
    <div className="border-b border-zinc-900 px-3 py-2">
      <div className="flex items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900 px-3">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          className="h-4 w-4 shrink-0 text-zinc-500"
        >
          <circle cx="11" cy="11" r="7" />
          <path d="m21 21-4.3-4.3" />
        </svg>
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Filter channels…"
          data-testid="channel-filter-input"
          className="h-9 w-full bg-transparent text-sm text-zinc-100 placeholder-zinc-500 outline-none"
        />
        {text !== "" && (
          <button
            onClick={() => setText("")}
            aria-label="Clear filter"
            data-testid="channel-filter-clear"
            className="shrink-0 text-zinc-500 transition-colors hover:text-zinc-200"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="h-4 w-4"
            >
              <path d="M18 6 6 18M6 6l12 12" />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}
