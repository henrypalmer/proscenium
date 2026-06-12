import { useEffect, useState } from "react";
import ContextMenu from "../components/common/ContextMenu";
import CategoryPanel from "../components/layout/CategoryPanel";
import ChannelList from "../components/live/ChannelList";
import * as api from "../lib/tauri";
import { useCatalogStore } from "../store/catalogStore";
import type { Category, LiveChannel } from "../types";

interface MenuState {
  channel: LiveChannel;
  x: number;
  y: number;
}

export default function LiveTV() {
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const refreshTick = useCatalogStore((s) => s.refreshTick);
  const notify = useCatalogStore((s) => s.notify);

  const [categories, setCategories] = useState<Category[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);

  const providerId = activeProvider?.id ?? null;

  useEffect(() => {
    if (!providerId) {
      setCategories([]);
      return;
    }
    let cancelled = false;
    void api.getLiveCategories(providerId).then(
      (cats) => {
        if (cancelled) return;
        setCategories(cats);
        // Drop a selection that disappeared with the latest refresh.
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
  }, [providerId, refreshTick]);

  if (!activeProvider) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <p className="text-sm font-medium text-zinc-400">No provider selected</p>
        <p className="max-w-xs text-xs text-zinc-600">
          Add or select a provider in Settings to browse live TV.
        </p>
      </div>
    );
  }

  // Playback ships in Milestone 4; the interactions exist now.
  const notYet = (channel: LiveChannel, what: string) =>
    notify(`${what} for "${channel.name}" arrives with the player in Milestone 4.`);

  return (
    <div className="flex h-full">
      <CategoryPanel
        title="Categories"
        allLabel="All Channels"
        categories={categories}
        selectedId={selected}
        onSelect={setSelected}
      />
      <div className="min-w-0 flex-1">
        <ChannelList
          providerId={activeProvider.id}
          categoryId={selected}
          showCategory={selected === null}
          version={refreshTick}
          onActivate={(channel) => notYet(channel, "Playback")}
          onContextMenu={(channel, x, y) => setMenu({ channel, x, y })}
        />
      </div>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            {
              label: "Play",
              onSelect: () => notYet(menu.channel, "Playback"),
            },
            {
              label: "Open in External Player",
              onSelect: () => notYet(menu.channel, "External player launch"),
            },
          ]}
        />
      )}
    </div>
  );
}
