import ProviderList from "../components/providers/ProviderList";

const SECTIONS = [
  { key: "providers", label: "Providers", enabled: true },
  { key: "playback", label: "Playback", enabled: false },
  { key: "appearance", label: "Appearance", enabled: false },
];

export default function Settings() {
  return (
    <div className="flex h-full">
      <nav className="w-44 shrink-0 border-r border-zinc-800 p-3">
        {SECTIONS.map((s) => (
          <button
            key={s.key}
            disabled={!s.enabled}
            title={s.enabled ? undefined : "Coming in a later milestone"}
            className={`block w-full rounded-md px-3 py-2 text-left text-sm ${
              s.enabled
                ? "bg-zinc-800 font-medium text-white"
                : "cursor-not-allowed text-zinc-600"
            }`}
          >
            {s.label}
          </button>
        ))}
      </nav>
      <div className="flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-3xl">
          <ProviderList />
        </div>
      </div>
    </div>
  );
}
