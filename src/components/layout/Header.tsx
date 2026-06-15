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
