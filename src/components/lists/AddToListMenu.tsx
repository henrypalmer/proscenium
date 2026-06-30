import { useEffect, useLayoutEffect, useRef, useState } from "react";
import * as api from "../../lib/tauri";
import { useWindowKeydown } from "../../lib/keyboard";
import { useCatalogStore } from "../../store/catalogStore";
import { useListsStore } from "../../store/listsStore";
import type { ListContentType } from "../../types";
import ListEditorDialog from "./ListEditorDialog";

interface AddToListMenuProps {
  providerId: string;
  contentType: ListContentType;
  contentId: string;
  x: number;
  y: number;
  onClose: () => void;
}

/**
 * "Add to list…" picker (spec §5.11): toggle the item's membership in each of
 * the user's lists, with an inline "+ New list…". Dismissible via click-away /
 * Esc. Reads/writes through the shared lists store so covers/counts stay fresh.
 */
export default function AddToListMenu({
  providerId,
  contentType,
  contentId,
  x,
  y,
  onClose,
}: AddToListMenuProps) {
  const lists = useListsStore((s) => s.lists);
  const loadLists = useListsStore((s) => s.load);
  const addItem = useListsStore((s) => s.addItem);
  const removeItem = useListsStore((s) => s.removeItem);
  const createList = useListsStore((s) => s.create);

  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  const [member, setMember] = useState<Set<string>>(new Set());
  const [creating, setCreating] = useState(false);

  // Load all lists (global since M39) and which already contain this item.
  useEffect(() => {
    void loadLists();
    void api
      .getListsForItem(providerId, contentType, contentId)
      .then((ids) => setMember(new Set(ids)), () => setMember(new Set()));
  }, [providerId, contentType, contentId, loadLists]);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    setPos({
      x: Math.min(x, window.innerWidth - rect.width - 8),
      y: Math.min(y, window.innerHeight - rect.height - 8),
    });
  }, [x, y, lists.length]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    // Defer so the opening click doesn't immediately dismiss it.
    const t = setTimeout(() => window.addEventListener("mousedown", onDown), 0);
    return () => {
      clearTimeout(t);
      window.removeEventListener("mousedown", onDown);
    };
  }, [onClose]);

  // Esc dismisses the picker (Milestone 23).
  useWindowKeydown(
    (e) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );

  const toggle = async (listId: string) => {
    const next = new Set(member);
    if (next.has(listId)) {
      next.delete(listId);
      setMember(next);
      await removeItem(listId, providerId, contentType, contentId);
    } else {
      next.add(listId);
      setMember(next);
      await addItem(listId, providerId, contentType, contentId);
      // Milestone 24: confirm the add with a toast (the checkbox alone is easy
      // to miss inside the still-open picker).
      const name = lists.find((l) => l.id === listId)?.name ?? "list";
      useCatalogStore.getState().notify(`Added to ${name}.`);
    }
  };

  const onCreate = async (name: string) => {
    setCreating(false);
    const created = await createList(name);
    if (created) {
      setMember((m) => new Set(m).add(created.id));
      await addItem(created.id, providerId, contentType, contentId);
      useCatalogStore.getState().notify(`Added to ${created.name}.`);
    }
    onClose();
  };

  if (creating) {
    return (
      <ListEditorDialog
        title="New list"
        submitLabel="Create & add"
        onSubmit={onCreate}
        onClose={() => setCreating(false)}
      />
    );
  }

  return (
    <div
      ref={ref}
      role="menu"
      data-testid="add-to-list-menu"
      style={{ left: pos.x, top: pos.y }}
      className="fixed z-[55] max-h-80 w-60 overflow-y-auto rounded-md border border-zinc-700 bg-zinc-900 py-1 shadow-xl"
    >
      <p className="px-3 py-1.5 text-xs font-medium uppercase tracking-wide text-zinc-500">
        Add to list
      </p>
      {lists.length === 0 && (
        <p className="px-3 py-1.5 text-sm text-zinc-500">No lists yet</p>
      )}
      {lists.map((list) => {
        const checked = member.has(list.id);
        return (
          <button
            key={list.id}
            role="menuitemcheckbox"
            aria-checked={checked}
            data-testid="add-to-list-option"
            onClick={() => void toggle(list.id)}
            className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-zinc-200 hover:bg-zinc-800"
          >
            <span
              className={`flex h-4 w-4 shrink-0 items-center justify-center rounded border text-[10px] ${
                checked
                  ? "border-emerald-500 bg-emerald-500 text-white"
                  : "border-zinc-600"
              }`}
            >
              {checked ? "✓" : ""}
            </span>
            <span className="min-w-0 flex-1 truncate">{list.name}</span>
          </button>
        );
      })}
      <div className="my-1 border-t border-zinc-800" />
      <button
        role="menuitem"
        data-testid="add-to-list-new"
        onClick={() => setCreating(true)}
        className="block w-full px-3 py-1.5 text-left text-sm font-medium text-zinc-100 hover:bg-zinc-800"
      >
        + New list…
      </button>
    </div>
  );
}
