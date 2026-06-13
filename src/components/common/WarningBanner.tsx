import { useState } from "react";
import { useCatalogStore } from "../../store/catalogStore";

/**
 * Persistent inline banner for active-provider problems (spec §12): the
 * provider was unreachable at startup, or an Xtream subscription has expired.
 * Unreachable banners offer a Retry; both can be dismissed.
 */
export default function WarningBanner() {
  const status = useCatalogStore((s) => s.providerStatus);
  const recheck = useCatalogStore((s) => s.recheckProviderStatus);
  const dismiss = useCatalogStore((s) => s.dismissProviderStatus);
  const [retrying, setRetrying] = useState(false);

  if (!status || (status.reachable && !status.expired)) return null;

  const expired = status.expired;
  const message =
    status.message ??
    (expired
      ? "Your subscription has expired."
      : "Your provider is currently unreachable. Showing cached content.");

  return (
    <div
      role="alert"
      data-testid="warning-banner"
      className="flex items-center gap-3 border-b border-amber-900/60 bg-amber-950/40 px-6 py-2.5 text-sm text-amber-200"
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        className="h-4 w-4 shrink-0"
      >
        <path d="M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" />
      </svg>
      <span className="min-w-0 flex-1">{message}</span>
      {!expired && (
        <button
          onClick={async () => {
            setRetrying(true);
            try {
              await recheck();
            } finally {
              setRetrying(false);
            }
          }}
          disabled={retrying}
          className="shrink-0 rounded-md border border-amber-700/70 px-3 py-1 text-xs font-medium text-amber-100 hover:bg-amber-900/40 disabled:opacity-60"
        >
          {retrying ? "Retrying…" : "Retry"}
        </button>
      )}
      <button
        onClick={dismiss}
        aria-label="Dismiss warning"
        className="shrink-0 text-amber-400/80 hover:text-amber-100"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-4 w-4">
          <path d="M6 6l12 12M18 6L6 18" />
        </svg>
      </button>
    </div>
  );
}
