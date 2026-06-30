import { useEffect, useState } from "react";
import ContextMenu from "../components/common/ContextMenu";
import AddToListMenu from "../components/lists/AddToListMenu";
import CategoryPanel from "../components/layout/CategoryPanel";
import ChannelFilterBar from "../components/live/ChannelFilterBar";
import ChannelList from "../components/live/ChannelList";
import RecentChannelsRow from "../components/live/RecentChannelsRow";
import * as api from "../lib/tauri";
import { multiViewSupported } from "../lib/tauri";
import { useCatalogStore } from "../store/catalogStore";
import { usePlayerStore } from "../store/playerStore";
import { useMultiViewStore } from "../store/multiViewStore";
import type { Category, LiveChannel } from "../types";

interface MenuState {
  channel: LiveChannel;
  x: number;
  y: number;
}

export default function LiveTV() {
  const providerIds = useCatalogStore((s) => s.providerIds);
  const refreshTick = useCatalogStore((s) => s.refreshTick);
  const notify = useCatalogStore((s) => s.notify);

  const [categories, setCategories] = useState<Category[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [addTo, setAddTo] = useState<{
    id: string;
    providerId: string;
    x: number;
    y: number;
  } | null>(null);

  const hasProviders = providerIds.length > 0;
  const scopeKey = providerIds.join(",");

  // Spec §5.3: the channel filter resets when the provider set changes.
  useEffect(() => {
    setFilter("");
  }, [scopeKey]);

  useEffect(() => {
    if (!hasProviders) {
      setCategories([]);
      return;
    }
    let cancelled = false;
    void api.getLiveCategories(providerIds).then(
      (cats) => {
        if (cancelled) return;
        setCategories(cats);
        setSelected((current) =>
          current && !cats.some((c) => c.id === current) ? null : current,
        );
      },
      () => {
        if (!cancelled) setCategories([]);
      },
    );
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKey, refreshTick]);

  if (!hasProviders) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <p className="text-sm font-medium text-zinc-400">No provider enabled</p>
        <p className="max-w-xs text-xs text-zinc-600">
          Add or enable a provider in Settings to browse live TV.
        </p>
      </div>
    );
  }

  const play = (channel: LiveChannel) =>
    void usePlayerStore.getState().openContent({
      providerId: channel.providerId,
      contentType: "live",
      contentId: channel.id,
      title: channel.name,
    });
  const openExternal = async (channel: LiveChannel) => {
    try {
      const url = await api.resolveStreamUrl(channel.providerId, "live", channel.id);
      await api.openInExternalPlayer(url);
    } catch (e) {
      notify(String(e), "error");
    }
  };

  return (
    <div className="flex h-full">
      <CategoryPanel
        title="Categories"
        allLabel="All Channels"
        categories={categories}
        selectedId={selected}
        onSelect={setSelected}
        providerIds={providerIds}
        section="live"
      />
      <div className="flex min-w-0 flex-1 flex-col">
        <ChannelFilterBar key={scopeKey} onQueryChange={setFilter} />
        {/* "Recently watched" strip on the landing (All Channels, no active
            filter) — spec §13, Milestone 29; merged across providers. */}
        {selected === null && filter === "" && (
          <RecentChannelsRow
            providerIds={providerIds}
            refreshKey={refreshTick}
            onActivate={play}
          />
        )}
        {/* Global-scope hint (spec §13, QA §2): the filter is scoped to the
            selected category, so offer a one-click jump to search every channel. */}
        {selected !== null && filter !== "" && (
          <button
            onClick={() => setSelected(null)}
            data-testid="search-all-channels"
            className="flex items-center gap-1.5 border-b border-zinc-900 px-4 py-1.5 text-left text-xs text-zinc-500 hover:bg-zinc-900 hover:text-zinc-300"
          >
            Filtering within{" "}
            <span className="font-medium text-zinc-300">
              {categories.find((c) => c.id === selected)?.name ?? "this category"}
            </span>
            . Search all channels →
          </button>
        )}
        <div className="min-h-0 flex-1">
          <ChannelList
            providerIds={providerIds}
            categoryId={selected}
            showCategory={selected === null}
            version={refreshTick}
            query={filter}
            onActivate={play}
            onContextMenu={(channel, x, y) => setMenu({ channel, x, y })}
          />
        </div>
      </div>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { label: "Play", onSelect: () => play(menu.channel) },
            ...(multiViewSupported
              ? [
                  {
                    label: "Add to Multi-view",
                    onSelect: () =>
                      void useMultiViewStore.getState().addFromList({
                        providerId: menu.channel.providerId,
                        contentId: menu.channel.id,
                        title: menu.channel.name,
                        logoUrl: menu.channel.logoUrl,
                      }),
                  },
                ]
              : []),
            {
              label: "Open in External Player",
              onSelect: () => void openExternal(menu.channel),
            },
            {
              label: "Add to list…",
              onSelect: () =>
                setAddTo({
                  id: menu.channel.id,
                  providerId: menu.channel.providerId,
                  x: menu.x,
                  y: menu.y,
                }),
            },
          ]}
        />
      )}
      {addTo && (
        <AddToListMenu
          providerId={addTo.providerId}
          contentType="live"
          contentId={addTo.id}
          x={addTo.x}
          y={addTo.y}
          onClose={() => setAddTo(null)}
        />
      )}
    </div>
  );
}
