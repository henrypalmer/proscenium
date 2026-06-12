import { useEffect } from "react";
import { useCatalogStore } from "../../store/catalogStore";

const AUTO_DISMISS_MS = 8000;

/** Non-blocking notification, bottom-right (spec §5.2 refresh failures). */
export default function Toast() {
  const toast = useCatalogStore((s) => s.toast);
  const dismiss = useCatalogStore((s) => s.dismissToast);

  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(dismiss, AUTO_DISMISS_MS);
    return () => window.clearTimeout(timer);
  }, [toast, dismiss]);

  if (!toast) return null;

  const isError = toast.kind === "error";

  return (
    <div
      className={`fixed bottom-4 right-4 z-50 flex max-w-md items-start gap-3 rounded-lg border bg-zinc-900 px-4 py-3 shadow-lg ${
        isError ? "border-red-900" : "border-zinc-700"
      }`}
    >
      <p className={`text-sm ${isError ? "text-red-300" : "text-zinc-200"}`}>
        {toast.message}
      </p>
      <button
        onClick={dismiss}
        aria-label="Dismiss notification"
        className="shrink-0 text-zinc-500 hover:text-zinc-200"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-4 w-4">
          <path d="M6 6l12 12M18 6L6 18" />
        </svg>
      </button>
    </div>
  );
}
