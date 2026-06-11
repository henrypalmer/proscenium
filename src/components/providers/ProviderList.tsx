import { useState } from "react";
import { useProviderStore } from "../../store/providerStore";
import type { Provider } from "../../types";
import ProviderCard from "./ProviderCard";
import ProviderForm from "./ProviderForm";

type EditTarget = { mode: "new" } | { mode: "edit"; provider: Provider } | null;

export default function ProviderList() {
  const providers = useProviderStore((s) => s.providers);
  const [editing, setEditing] = useState<EditTarget>(null);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-zinc-200">Providers</h3>
        {!editing && (
          <button
            onClick={() => setEditing({ mode: "new" })}
            className="rounded-md bg-zinc-100 px-3 py-1.5 text-xs font-semibold text-zinc-900 hover:bg-white"
          >
            Add Provider
          </button>
        )}
      </div>

      {editing && (
        <div className="rounded-lg border border-zinc-800 bg-zinc-900/60 p-5">
          <h4 className="mb-4 text-sm font-semibold text-zinc-100">
            {editing.mode === "new" ? "Add Provider" : "Edit Provider"}
          </h4>
          <ProviderForm
            initial={editing.mode === "edit" ? editing.provider : null}
            onSaved={() => setEditing(null)}
            onCancel={() => setEditing(null)}
          />
        </div>
      )}

      {providers.length === 0 && !editing ? (
        <p className="rounded-lg border border-dashed border-zinc-800 p-6 text-center text-sm text-zinc-500">
          No providers configured yet.
        </p>
      ) : (
        <div className="flex flex-col gap-3">
          {providers.map((p) => (
            <ProviderCard
              key={p.id}
              provider={p}
              onEdit={() => setEditing({ mode: "edit", provider: p })}
            />
          ))}
        </div>
      )}
    </div>
  );
}
