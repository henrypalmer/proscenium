import { useLocation } from "react-router-dom";
import { useCatalogStore } from "../../store/catalogStore";
import { useProviderStore } from "../../store/providerStore";

const TITLES: Record<string, string> = {
  "/live": "Live TV",
  "/movies": "Movies",
  "/shows": "TV Shows",
  "/settings": "Settings",
};

export default function Header() {
  const { pathname } = useLocation();
  const providers = useProviderStore((s) => s.providers);
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const refreshing = useCatalogStore((s) => s.refreshing);
  const stage = useCatalogStore((s) => s.stage);
  const progress = useCatalogStore((s) => s.progress);
  const refresh = useCatalogStore((s) => s.refresh);

  const provider = activeProvider ?? providers[0] ?? null;

  return (
    <header className="relative flex h-14 shrink-0 items-center justify-between border-b border-zinc-800 bg-zinc-950 px-6">
      <h2 className="text-base font-semibold text-zinc-100">
        {TITLES[pathname] ?? "Proscenium"}
      </h2>
      <div className="flex items-center gap-3">
        {refreshing && stage && (
          <span className="text-xs text-zinc-500">{stage}</span>
        )}
        {provider && (
          <span className="rounded-full border border-zinc-700 px-3 py-1 text-xs text-zinc-300">
            {provider.name}
          </span>
        )}
        <button
          onClick={() => void refresh()}
          disabled={!provider || refreshing}
          title={refreshing ? "Refresh in progress" : "Refresh catalog"}
          className="rounded-md p-2 text-zinc-400 transition-colors hover:bg-zinc-900 hover:text-zinc-100 disabled:cursor-not-allowed disabled:hover:bg-transparent"
          aria-label="Refresh catalog"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            className={`h-5 w-5 ${refreshing ? "animate-spin" : ""}`}
          >
            <path d="M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6" />
          </svg>
        </button>
      </div>
      {refreshing && (
        <div className="absolute inset-x-0 bottom-0 h-0.5 bg-zinc-800">
          <div
            className="h-full bg-zinc-400 transition-all duration-300"
            style={{ width: `${Math.max(4, Math.round(progress * 100))}%` }}
          />
        </div>
      )}
    </header>
  );
}
