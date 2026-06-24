import { useWindowKeydown } from "../../lib/keyboard";

interface ConfirmDialogProps {
  title: string;
  message?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  /** Style the confirm button as a destructive action. */
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Shared confirmation modal (spec §1/§6, Milestone 24) for destructive actions.
 * Dismissible via click-away / Esc; Enter confirms. Rendered above other modals
 * (`z-[70]`).
 */
export default function ConfirmDialog({
  title,
  message,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  danger = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  useWindowKeydown(
    (e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      } else if (e.key === "Enter") {
        e.preventDefault();
        onConfirm();
      }
    },
    [onCancel, onConfirm],
  );

  return (
    <div
      data-testid="confirm-dialog"
      className="fixed inset-0 z-[70] flex items-center justify-center bg-black/70 p-6"
      onClick={onCancel}
    >
      <div
        className="w-full max-w-sm rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold text-white">{title}</h2>
        {message && <p className="mt-2 text-sm text-zinc-400">{message}</p>}
        <div className="mt-5 flex justify-end gap-2">
          <button
            onClick={onCancel}
            className="rounded-md border border-zinc-700 px-4 py-2 text-sm font-medium text-zinc-200 hover:bg-zinc-800"
          >
            {cancelLabel}
          </button>
          <button
            autoFocus
            onClick={onConfirm}
            data-testid="confirm-dialog-confirm"
            className={`rounded-md px-4 py-2 text-sm font-semibold ${
              danger
                ? "bg-rose-600 text-white hover:bg-rose-500"
                : "bg-zinc-100 text-zinc-900 hover:bg-white"
            }`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
