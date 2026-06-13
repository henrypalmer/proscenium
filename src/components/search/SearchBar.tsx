import { useEffect, useRef, useState } from "react";
import type { Category, SearchContentType } from "../../types";

/** Spec §5.5: results appear as the user types, debounced ~200ms. */
const DEBOUNCE_MS = 200;

const TABS: { type: SearchContentType; label: string }[] = [
  { type: "all", label: "All" },
  { type: "live", label: "Live TV" },
  { type: "movies", label: "Movies" },
  { type: "series", label: "TV Shows" },
];

interface SearchBarProps {
  /** Called with the trimmed query after the debounce delay. */
  onQueryChange: (query: string) => void;
  contentType: SearchContentType;
  onContentTypeChange: (type: SearchContentType) => void;
  /** Genres for the selected content type ([] when type is "all"). */
  categories: Category[];
  categoryId: string | null;
  onCategoryChange: (categoryId: string | null) => void;
}

/** Debounced search input plus content type filter tabs and, when a
 * specific type is selected, a genre/category narrowing select. */
export default function SearchBar({
  onQueryChange,
  contentType,
  onContentTypeChange,
  categories,
  categoryId,
  onCategoryChange,
}: SearchBarProps) {
  const [text, setText] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => onQueryChange(text.trim()), DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [text, onQueryChange]);

  return (
    <div className="border-b border-zinc-800 p-4">
      <div className="flex items-center gap-3 rounded-lg border border-zinc-700 bg-zinc-900 px-3">
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
          ref={inputRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Search channels, movies, and shows…"
          data-testid="search-input"
          className="h-11 w-full bg-transparent text-sm text-zinc-100 placeholder-zinc-500 outline-none"
        />
      </div>
      <div className="mt-3 flex items-center gap-1.5">
        {TABS.map(({ type, label }) => (
          <button
            key={type}
            onClick={() => onContentTypeChange(type)}
            data-testid={`search-tab-${type}`}
            className={`rounded-full px-3 py-1 text-xs transition-colors ${
              contentType === type
                ? "bg-zinc-100 font-semibold text-zinc-900"
                : "bg-zinc-900 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
            }`}
          >
            {label}
          </button>
        ))}
        {contentType !== "all" && categories.length > 0 && (
          <select
            value={categoryId ?? ""}
            onChange={(e) => onCategoryChange(e.target.value || null)}
            data-testid="search-category-select"
            className="ml-auto max-w-48 rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs text-zinc-300 outline-none"
          >
            <option value="">
              {contentType === "live" ? "All categories" : "All genres"}
            </option>
            {categories.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        )}
      </div>
    </div>
  );
}
