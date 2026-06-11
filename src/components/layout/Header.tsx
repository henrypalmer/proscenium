import { useLocation } from "react-router-dom";
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
  // Active-provider selection lands in Milestone 2; until then show the first profile.
  const provider = providers[0] ?? null;

  return (
    <header className="flex h-14 shrink-0 items-center justify-between border-b border-zinc-800 bg-zinc-950 px-6">
      <h2 className="text-base font-semibold text-zinc-100">
        {TITLES[pathname] ?? "Proscenium"}
      </h2>
      <div className="flex items-center gap-3">
        {provider && (
          <span className="rounded-full border border-zinc-700 px-3 py-1 text-xs text-zinc-300">
            {provider.name}
          </span>
        )}
        <button
          disabled
          title="Catalog refresh arrives in Milestone 2"
          className="cursor-not-allowed rounded-md p-2 text-zinc-600"
          aria-label="Refresh catalog"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
            <path d="M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6" />
          </svg>
        </button>
      </div>
    </header>
  );
}
