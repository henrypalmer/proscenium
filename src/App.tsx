import { useEffect } from "react";
import {
  BrowserRouter,
  Navigate,
  Route,
  Routes,
} from "react-router-dom";
import TopNav from "./components/layout/TopNav";
import ProviderForm from "./components/providers/ProviderForm";
import Toast from "./components/common/Toast";
import WarningBanner from "./components/common/WarningBanner";
import PlayerOverlay from "./components/player/PlayerOverlay";
import MultiView from "./components/player/MultiView";
import ResumeDialog from "./components/player/ResumeDialog";
import SearchOverlay from "./components/search/SearchOverlay";
import SearchResultsPage from "./components/search/SearchResultsPage";
import Home from "./pages/Home";
import LiveTV from "./pages/LiveTV";
import ListDetail from "./pages/ListDetail";
import Movies from "./pages/Movies";
import Settings from "./pages/Settings";
import TVShows from "./pages/TVShows";
import { useCatalogStore } from "./store/catalogStore";
import { usePlayerStore } from "./store/playerStore";
import { useMultiViewStore } from "./store/multiViewStore";
import { useListsStore } from "./store/listsStore";
import { useProgressStore } from "./store/progressStore";
import { useProviderStore } from "./store/providerStore";
import { useSettingsStore } from "./store/settingsStore";
import { checkForUpdatesOnLaunch } from "./lib/updater";

function FirstLaunch() {
  return (
    <main className="flex h-full items-center justify-center overflow-y-auto p-6">
      <div className="w-full max-w-lg rounded-xl border border-zinc-800 bg-zinc-900/60 p-8">
        <h1 className="text-xl font-semibold text-white">
          Welcome to Proscenium
        </h1>
        <p className="mt-1 mb-6 text-sm text-zinc-400">
          Add your IPTV provider to get started.
        </p>
        <ProviderForm onSaved={() => undefined} />
      </div>
    </main>
  );
}

function Shell() {
  // While the player is open, the browser stays mounted (state, scroll, and
  // selections survive) but stops painting so the native video can show
  // through the transparent page background.
  const playerOpen = usePlayerStore((s) => s.open);
  // Changing the enabled provider set (Milestone 39) keeps the user on the same
  // section but remounts its page, so per-provider state (selected genre,
  // filters, scroll) resets and any open detail overlay closes — landing on the
  // section's main screen without navigating away.
  const scopeKey = useCatalogStore((s) => s.providerIds.join(","));
  return (
    <div className={`flex h-full min-w-0 flex-col ${playerOpen ? "invisible" : ""}`}>
      <WarningBanner />
      {/* Positioning context for the floating top-center nav (spec §9). */}
      <div className="relative min-h-0 flex-1">
        <TopNav />
        <main className="h-full overflow-y-auto pt-16">
          <div key={scopeKey || "no-provider"} className="h-full">
            <Routes>
              <Route path="/" element={<Home />} />
              <Route path="/live" element={<LiveTV />} />
              <Route path="/movies" element={<Movies />} />
              <Route path="/shows" element={<TVShows />} />
              <Route path="/list/:listId" element={<ListDetail />} />
              <Route path="/search" element={<SearchResultsPage />} />
              <Route path="/settings" element={<Settings />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </div>
        </main>
      </div>
      <SearchOverlay />
    </div>
  );
}

export default function App() {
  const loaded = useProviderStore((s) => s.loaded);
  const providers = useProviderStore((s) => s.providers);
  const load = useProviderStore((s) => s.load);

  useEffect(() => {
    void (async () => {
      await load();
      await useSettingsStore.getState().load();
      await useCatalogStore
        .getState()
        .init(useProviderStore.getState().providers);
      // Spec §13: check for app updates on launch (no-op in the browser).
      void checkForUpdatesOnLaunch();
    })();
    // Dev/e2e hook: lets tooling inspect and drive the stores.
    (window as unknown as Record<string, unknown>).__proscenium = {
      player: usePlayerStore,
      catalog: useCatalogStore,
      providers: useProviderStore,
      progress: useProgressStore,
      lists: useListsStore,
      multiView: useMultiViewStore,
    };
  }, [load]);

  if (!loaded) {
    return <div className="h-full bg-zinc-950" />;
  }

  return (
    <BrowserRouter>
      {providers.length === 0 ? <FirstLaunch /> : <Shell />}
      <PlayerOverlay />
      <MultiView />
      <ResumeDialog />
      <Toast />
    </BrowserRouter>
  );
}
