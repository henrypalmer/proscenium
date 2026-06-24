import { useState } from "react";
import { testProviderConnection } from "../../lib/tauri";
import { formatUnixDate } from "../../lib/utils";
import { useProviderStore } from "../../store/providerStore";
import ConfirmDialog from "../common/ConfirmDialog";
import type { ConnectionTestResult, Provider } from "../../types";

interface ProviderCardProps {
  provider: Provider;
  onEdit: () => void;
}

export default function ProviderCard({ provider, onEdit }: ProviderCardProps) {
  const remove = useProviderStore((s) => s.remove);
  const [status, setStatus] = useState<ConnectionTestResult | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const endpoint =
    provider.serverUrl ?? provider.playlistUrl ?? provider.localFilePath ?? "";

  async function handleCheckStatus() {
    setChecking(true);
    setError(null);
    try {
      // For saved Xtream providers the backend pulls the password from the OS keychain.
      setStatus(
        await testProviderConnection({
          id: provider.id,
          name: provider.name,
          type: provider.type,
          serverUrl: provider.serverUrl ?? undefined,
          username: provider.username ?? undefined,
          playlistUrl: provider.playlistUrl ?? undefined,
          localFilePath: provider.localFilePath ?? undefined,
        }),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setChecking(false);
    }
  }

  async function handleDelete() {
    setConfirmingDelete(false);
    try {
      await remove(provider.id);
    } catch (e) {
      setError(String(e));
    }
  }

  const expired = status?.accountInfo?.status?.toLowerCase() === "expired";

  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/60 p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-sm font-semibold text-zinc-100">
              {provider.name}
            </h3>
            <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-zinc-400">
              {provider.type === "xtream" ? "Xtream" : "M3U"}
            </span>
          </div>
          <p className="mt-1 truncate text-xs text-zinc-500" title={endpoint}>
            {endpoint}
          </p>
          <p className="mt-1 text-xs text-zinc-500">
            Last refreshed: {formatUnixDate(provider.lastRefreshed)}
          </p>
        </div>
        <div className="flex shrink-0 gap-1">
          <button
            onClick={() => void handleCheckStatus()}
            disabled={checking}
            className="rounded-md border border-zinc-700 px-2.5 py-1 text-xs text-zinc-300 hover:bg-zinc-800 disabled:opacity-50"
          >
            {checking ? "Checking…" : "Check Status"}
          </button>
          <button
            onClick={onEdit}
            className="rounded-md border border-zinc-700 px-2.5 py-1 text-xs text-zinc-300 hover:bg-zinc-800"
          >
            Edit
          </button>
          <button
            onClick={() => setConfirmingDelete(true)}
            data-testid="delete-provider"
            className="rounded-md border border-red-900/60 px-2.5 py-1 text-xs text-red-400 hover:bg-red-950/40"
          >
            Delete
          </button>
        </div>
      </div>

      {expired && (
        <p className="mt-3 rounded-md border border-amber-900 bg-amber-950/40 px-3 py-2 text-xs text-amber-300">
          This subscription has lapsed. Renew it with your provider to keep
          watching.
        </p>
      )}

      {status && (
        <div
          className={`mt-3 rounded-md border px-3 py-2 text-xs ${
            status.success
              ? "border-emerald-900 bg-emerald-950/40 text-emerald-300"
              : "border-red-900 bg-red-950/40 text-red-300"
          }`}
        >
          <p>{status.message}</p>
          {status.accountInfo && (
            <ul className="mt-1 text-zinc-400">
              {status.accountInfo.status && (
                <li>Status: {status.accountInfo.status}</li>
              )}
              {status.accountInfo.expDate !== null && (
                <li>Expires: {formatUnixDate(status.accountInfo.expDate)}</li>
              )}
              {status.accountInfo.maxConnections !== null && (
                <li>
                  Connections: {status.accountInfo.activeConnections ?? 0} /{" "}
                  {status.accountInfo.maxConnections} active
                </li>
              )}
            </ul>
          )}
        </div>
      )}

      {error && (
        <p className="mt-3 rounded-md border border-red-900 bg-red-950/40 px-3 py-2 text-xs text-red-300">
          {error}
        </p>
      )}

      {confirmingDelete && (
        <ConfirmDialog
          title={`Delete provider "${provider.name}"?`}
          message="All of its cached catalog data, lists, and watch history will be removed. This cannot be undone."
          confirmLabel="Delete provider"
          danger
          onConfirm={() => void handleDelete()}
          onCancel={() => setConfirmingDelete(false)}
        />
      )}
    </div>
  );
}
