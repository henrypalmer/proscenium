import { useState } from "react";
import ProviderList from "../components/providers/ProviderList";
import { useSettingsStore } from "../store/settingsStore";
import type { ExternalPlayer, UiDensity } from "../types";

const SECTIONS = [
  { key: "providers", label: "Providers" },
  { key: "playback", label: "Playback" },
  { key: "appearance", label: "Appearance" },
] as const;

type SectionKey = (typeof SECTIONS)[number]["key"];

export default function Settings() {
  const [section, setSection] = useState<SectionKey>("providers");

  return (
    <div className="flex h-full">
      <nav className="w-44 shrink-0 border-r border-zinc-800 p-3">
        {SECTIONS.map((s) => (
          <button
            key={s.key}
            onClick={() => setSection(s.key)}
            className={`block w-full rounded-md px-3 py-2 text-left text-sm ${
              section === s.key
                ? "bg-zinc-800 font-medium text-white"
                : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200"
            }`}
          >
            {s.label}
          </button>
        ))}
      </nav>
      <div className="flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-3xl">
          {section === "providers" && <ProviderList />}
          {section === "playback" && <PlaybackSettings />}
          {section === "appearance" && <AppearanceSettings />}
        </div>
      </div>
    </div>
  );
}

function Row({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-zinc-800/70 py-4 last:border-0">
      <div className="min-w-0">
        <div className="text-sm font-medium text-zinc-200">{label}</div>
        {description && (
          <div className="mt-0.5 text-xs text-zinc-500">{description}</div>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function PlaybackSettings() {
  const settings = useSettingsStore((s) => s.settings);
  const update = useSettingsStore((s) => s.update);
  if (!settings) return null;

  return (
    <div>
      <h3 className="mb-2 text-sm font-semibold text-zinc-200">Playback</h3>
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/40 px-5">
        <Row
          label="Default external player"
          description="Used by “Open in External Player”."
        >
          <select
            data-testid="default-player-select"
            value={settings.defaultExternalPlayer}
            onChange={(e) =>
              void update("defaultExternalPlayer", e.target.value as ExternalPlayer)
            }
            className="rounded-md border border-zinc-700 bg-zinc-800 px-3 py-1.5 text-sm text-zinc-100"
          >
            <option value="mpv">mpv</option>
            <option value="vlc">VLC</option>
            <option value="custom">Custom…</option>
          </select>
        </Row>

        {settings.defaultExternalPlayer === "custom" && (
          <Row
            label="Custom player command"
            description="Use {url} as the stream URL placeholder."
          >
            <input
              type="text"
              spellCheck={false}
              placeholder='e.g. potplayer "{url}"'
              value={settings.customPlayerCommand ?? ""}
              onChange={(e) => void update("customPlayerCommand", e.target.value)}
              className="w-72 rounded-md border border-zinc-700 bg-zinc-800 px-3 py-1.5 text-sm text-zinc-100 placeholder:text-zinc-600"
            />
          </Row>
        )}

        <Row
          label="Hardware decode"
          description="Use the GPU video decoder where available. Takes effect on the next stream."
        >
          <Toggle
            testId="hw-decode-toggle"
            checked={settings.hwDecodeEnabled}
            onChange={(v) => void update("hwDecodeEnabled", v)}
          />
        </Row>
      </div>
    </div>
  );
}

function AppearanceSettings() {
  const settings = useSettingsStore((s) => s.settings);
  const update = useSettingsStore((s) => s.update);
  if (!settings) return null;

  return (
    <div>
      <h3 className="mb-2 text-sm font-semibold text-zinc-200">Appearance</h3>
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/40 px-5">
        <Row label="Density" description="Spacing of content cards and lists.">
          <div className="flex overflow-hidden rounded-md border border-zinc-700">
            {(["comfortable", "compact"] as UiDensity[]).map((d) => (
              <button
                key={d}
                data-testid={`density-${d}`}
                onClick={() => void update("uiDensity", d)}
                className={`px-3 py-1.5 text-sm capitalize ${
                  settings.uiDensity === d
                    ? "bg-zinc-100 font-medium text-zinc-900"
                    : "bg-zinc-800 text-zinc-300 hover:bg-zinc-700"
                }`}
              >
                {d}
              </button>
            ))}
          </div>
        </Row>

        <Row
          label="Theme"
          description="Light theme is planned for a future release."
        >
          {/* A status label, not a control — Dark is currently the only theme
              (Milestone 24), so it must not look clickable like the Density buttons. */}
          <span
            data-testid="theme-status"
            className="inline-flex items-center gap-2 text-sm text-zinc-300"
          >
            <span className="h-2.5 w-2.5 rounded-full bg-zinc-100" aria-hidden />
            Dark
            <span className="text-xs text-zinc-600">· only theme</span>
          </span>
        </Row>
      </div>
    </div>
  );
}

function Toggle({
  checked,
  onChange,
  testId,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  testId?: string;
}) {
  // Flex-centered knob (Headless-UI pattern): `items-center` handles the
  // vertical centering and `translate-x` the horizontal slide, so the knob stays
  // contained with symmetric 2px margins. (The previous `absolute` knob had no
  // `left`, so its horizontal position relied on the static position and
  // rendered off-track.)
  return (
    <button
      role="switch"
      aria-checked={checked}
      data-testid={testId}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${
        checked ? "bg-emerald-600" : "bg-zinc-700"
      }`}
    >
      <span
        className={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform ${
          checked ? "translate-x-[22px]" : "translate-x-0.5"
        }`}
      />
    </button>
  );
}
