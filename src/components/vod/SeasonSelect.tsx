import { useEffect, useRef, useState } from "react";

interface SeasonSelectProps {
  seasons: number[];
  value: number;
  onChange: (season: number) => void;
}

/**
 * Season picker for the series detail view (spec §5.4, M20): a single dropdown
 * showing the current season, replacing the previous strip of per-season
 * buttons. Renders even for a single-season series for layout consistency.
 * Carries `data-testid="season-selector"` so it stands in for the old strip.
 */
export default function SeasonSelect({
  seasons,
  value,
  onChange,
}: SeasonSelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        setOpen(false);
      }
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative inline-block">
      <button
        type="button"
        data-testid="season-selector"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="flex min-w-[9rem] items-center justify-between gap-3 rounded-md border border-zinc-700 bg-zinc-900 px-3 py-1.5 text-sm font-medium text-zinc-100 hover:bg-zinc-800"
      >
        <span>Season {value}</span>
        <span
          className={`text-zinc-400 transition-transform ${open ? "rotate-180" : ""}`}
          aria-hidden
        >
          ▾
        </span>
      </button>
      {open && (
        <ul
          role="listbox"
          data-testid="season-options"
          className="absolute left-0 z-20 mt-1 max-h-72 min-w-[9rem] overflow-y-auto rounded-md border border-zinc-700 bg-zinc-900 py-1 shadow-xl"
        >
          {seasons.map((s) => (
            <li key={s}>
              <button
                type="button"
                role="option"
                aria-selected={s === value}
                data-testid="season-option"
                onClick={() => {
                  onChange(s);
                  setOpen(false);
                }}
                className={`block w-full px-3 py-1.5 text-left text-sm hover:bg-zinc-800 ${
                  s === value
                    ? "font-semibold text-zinc-100"
                    : "text-zinc-300"
                }`}
              >
                Season {s}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
