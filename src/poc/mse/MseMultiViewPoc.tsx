import { useEffect, useMemo, useState } from "react";
import Hls from "hls.js";
import mpegts from "mpegts.js";
import * as api from "../../lib/tauri";
import { useCatalogStore } from "../../store/catalogStore";
import type { LiveChannel } from "../../types";
import MseTile, { type TileDiag, type TileKind } from "./MseTile";
import { pocChannelUrl, pocProxyBase } from "./pocApi";

/**
 * Spike D POC: live-TV multi-view using MSE `<video>` tiles (mpegts.js / hls.js)
 * instead of native mpv windows. The whole point: with MSE, a multi-view grid is
 * just N `<video>` elements in a CSS grid — no native windows, no z-order glue,
 * trivial audio focus. Real provider channels stream through a local Rust proxy
 * (CORS + keychain compose); a manual URL field plays any CORS-enabled test
 * stream directly. Reachable at /poc/mse (POC branch only).
 */

const MAX_TILES = 4;

interface Stream {
  id: string;
  label: string;
  src: string;
  kind: TileKind;
}

function kindForExt(ext: string): TileKind {
  return /m3u8|hls/i.test(ext) ? "hls" : "ts";
}

export default function MseMultiViewPoc() {
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const providerId = activeProvider?.id ?? null;

  const [proxyBase, setProxyBase] = useState<string>("");
  const [streams, setStreams] = useState<Stream[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [picker, setPicker] = useState(false);
  const [manualUrl, setManualUrl] = useState("");
  const [diags, setDiags] = useState<Record<string, TileDiag>>({});

  useEffect(() => {
    void pocProxyBase().then(setProxyBase);
  }, []);

  // One-time WebView capability probe (the core Spike-D question).
  const support = useMemo(() => {
    const f = mpegts.getFeatureList();
    return {
      mse: typeof MediaSource !== "undefined",
      mpegtsLive: f.mseLivePlayback,
      hls: Hls.isSupported(),
      nativeHls:
        document.createElement("video").canPlayType("application/vnd.apple.mpegurl") !== "",
    };
  }, []);

  const canAdd = streams.length < MAX_TILES;

  const addStream = (s: Stream) => {
    setStreams((prev) => {
      if (prev.length >= MAX_TILES) return prev;
      const next = [...prev, s];
      return next;
    });
    setActiveId((cur) => cur ?? s.id);
  };

  const closeStream = (id: string) => {
    setStreams((prev) => prev.filter((s) => s.id !== id));
    setActiveId((cur) => (cur === id ? null : cur));
    setDiags((d) => {
      const { [id]: _drop, ...rest } = d;
      return rest;
    });
  };

  const addChannel = (channel: LiveChannel) => {
    if (!providerId || !proxyBase) return;
    addStream({
      id: `ch-${channel.id}-${Date.now()}`,
      label: channel.name || "Channel",
      src: pocChannelUrl(proxyBase, providerId, channel.id),
      kind: kindForExt(channel.streamExt),
    });
    setPicker(false);
  };

  const addManual = () => {
    const url = manualUrl.trim();
    if (!url) return;
    addStream({
      id: `url-${Date.now()}`,
      label: url.replace(/^https?:\/\//, "").slice(0, 40),
      src: url,
      kind: kindForExt(url),
    });
    setManualUrl("");
  };

  // Adaptive grid: 1 → full, 2 → side-by-side, 3/4 → 2×2.
  const cols = streams.length <= 1 ? 1 : 2;

  return (
    <div className="flex h-full flex-col gap-3 p-4">
      <header className="flex flex-wrap items-center gap-3">
        <h1 className="text-base font-semibold text-zinc-100">
          MSE Multi-View POC <span className="text-xs font-normal text-amber-400">(Spike D)</span>
        </h1>
        <div className="flex items-center gap-1.5 text-[11px]">
          {(
            [
              ["MSE", support.mse],
              ["mpegts.js live", support.mpegtsLive],
              ["hls.js", support.hls],
              ["native HLS", support.nativeHls],
            ] as const
          ).map(([label, ok]) => (
            <span
              key={label}
              className={`rounded px-1.5 py-0.5 ${
                ok ? "bg-emerald-500/15 text-emerald-300" : "bg-zinc-800 text-zinc-500"
              }`}
            >
              {ok ? "✓" : "✕"} {label}
            </span>
          ))}
        </div>
        <span className="ml-auto text-xs text-zinc-500">
          {streams.length} / {MAX_TILES} tiles · proxy {proxyBase ? "ready" : "unavailable (browser)"}
        </span>
      </header>

      {/* Add-stream controls. */}
      <div className="flex flex-wrap items-center gap-2">
        <button
          onClick={() => setPicker((p) => !p)}
          disabled={!canAdd || !providerId || !proxyBase}
          className="rounded-md bg-zinc-100 px-3 py-1.5 text-sm font-medium text-zinc-900 hover:bg-white disabled:opacity-40"
        >
          + Add channel
        </button>
        <div className="flex items-center gap-1">
          <input
            value={manualUrl}
            onChange={(e) => setManualUrl(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addManual()}
            placeholder="…or paste a stream URL (HLS/TS)"
            spellCheck={false}
            className="w-72 rounded-md border border-zinc-700 bg-zinc-800 px-3 py-1.5 text-sm text-zinc-100 placeholder:text-zinc-600"
          />
          <button
            onClick={addManual}
            disabled={!canAdd || !manualUrl.trim()}
            className="rounded-md border border-zinc-700 px-3 py-1.5 text-sm text-zinc-200 hover:bg-zinc-800 disabled:opacity-40"
          >
            Add URL
          </button>
        </div>
        {streams.length > 0 && (
          <button
            onClick={() => {
              setStreams([]);
              setActiveId(null);
              setDiags({});
            }}
            className="rounded-md border border-zinc-700 px-3 py-1.5 text-sm text-zinc-400 hover:bg-zinc-800"
          >
            Clear all
          </button>
        )}
      </div>

      {picker && (
        <ChannelPicker providerId={providerId} onPick={addChannel} onClose={() => setPicker(false)} />
      )}

      {/* The grid — the whole architectural point. */}
      {streams.length === 0 ? (
        <div className="flex flex-1 items-center justify-center rounded-lg border border-dashed border-zinc-800 text-center text-sm text-zinc-500">
          <div className="max-w-md">
            Add up to {MAX_TILES} live channels (real provider streams via the local proxy) or paste a
            test stream URL. Click a tile to give it audio.
          </div>
        </div>
      ) : (
        <div
          className="grid min-h-0 flex-1 gap-2"
          style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}
        >
          {streams.map((s) => (
            <MseTile
              key={s.id}
              src={s.src}
              kind={s.kind}
              label={s.label}
              active={activeId === s.id}
              onActivate={() => setActiveId(s.id)}
              onClose={() => closeStream(s.id)}
              onDiag={(d) => setDiags((prev) => ({ ...prev, [s.id]: d }))}
            />
          ))}
        </div>
      )}

      {/* Per-tile diagnostics summary. */}
      {streams.length > 0 && (
        <footer className="shrink-0 text-[11px] text-zinc-500">
          {streams.map((s) => {
            const d = diags[s.id];
            return (
              <span key={s.id} className="mr-4">
                {s.label}: {d ? `${d.state} · ${d.engine}${d.width ? ` · ${d.width}×${d.height}` : ""}${d.error ? ` · ${d.error}` : ""}` : "…"}
              </span>
            );
          })}
        </footer>
      )}
    </div>
  );
}

/** Minimal live-channel picker for the POC (reuses the cached catalog). */
function ChannelPicker({
  providerId,
  onPick,
  onClose,
}: {
  providerId: string | null;
  onPick: (channel: LiveChannel) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [channels, setChannels] = useState<LiveChannel[]>([]);

  useEffect(() => {
    if (!providerId) return;
    let cancelled = false;
    void api.getLiveChannels(providerId, undefined, query || undefined, 1, 40).then(
      (r) => !cancelled && setChannels(r.items),
      () => !cancelled && setChannels([]),
    );
    return () => {
      cancelled = true;
    };
  }, [providerId, query]);

  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/80 p-2">
      <div className="mb-2 flex items-center gap-2">
        <input
          autoFocus
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Filter channels…"
          className="flex-1 rounded-md border border-zinc-700 bg-zinc-800 px-3 py-1.5 text-sm text-zinc-100"
        />
        <button onClick={onClose} className="rounded-md px-2 py-1 text-sm text-zinc-400 hover:bg-zinc-800">
          Close
        </button>
      </div>
      <div className="grid max-h-48 grid-cols-2 gap-1 overflow-y-auto md:grid-cols-3">
        {channels.map((c) => (
          <button
            key={c.id}
            onClick={() => onPick(c)}
            className="truncate rounded-md px-2 py-1.5 text-left text-xs text-zinc-300 hover:bg-zinc-800"
            title={c.name}
          >
            {c.name || "Untitled channel"}
          </button>
        ))}
      </div>
    </div>
  );
}
