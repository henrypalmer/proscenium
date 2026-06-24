import { useState } from "react";
import { useListsStore } from "../../store/listsStore";
import ConfirmDialog from "../common/ConfirmDialog";
import ContextMenu from "../common/ContextMenu";
import ScrollRow from "../common/ScrollRow";
import ListEditorDialog from "../lists/ListEditorDialog";
import ListCoverCard from "./ListCoverCard";
import type { ListSummary } from "../../types";

interface MyListsRowProps {
  onOpenList: (listId: string) => void;
}

type Editor =
  | { mode: "create" }
  | { mode: "rename"; list: ListSummary }
  | null;

/**
 * Home "My Lists" row (spec §5.10): a horizontally-scrollable strip of list
 * cover cards led by a "+ New list" card. Always shown (even with no lists) so
 * the "+ New list" card is a discoverable entry point for creating the first
 * list. Reads the shared lists store, so it reflects changes made anywhere.
 */
export default function MyListsRow({ onOpenList }: MyListsRowProps) {
  const lists = useListsStore((s) => s.lists);
  const create = useListsStore((s) => s.create);
  const rename = useListsStore((s) => s.rename);
  const remove = useListsStore((s) => s.remove);

  const [editor, setEditor] = useState<Editor>(null);
  const [menu, setMenu] = useState<{ list: ListSummary; x: number; y: number } | null>(
    null,
  );
  const [deleting, setDeleting] = useState<ListSummary | null>(null);

  return (
    <section data-testid="home-my-lists">
      <h2 className="mb-3 text-base font-semibold text-zinc-200">
        My Lists
        {lists.length > 0 && (
          <span className="ml-2 text-sm font-normal text-zinc-600">{lists.length}</span>
        )}
      </h2>
      <ScrollRow>
        <button
          onClick={() => setEditor({ mode: "create" })}
          data-testid="new-list-card"
          className="prosc-enter relative flex aspect-[2/3] w-[180px] shrink-0 flex-col items-center justify-center rounded-lg border border-dashed border-zinc-700 text-zinc-400 transition duration-200 ease-out hover:z-10 hover:scale-[1.04] hover:border-zinc-500 hover:text-zinc-200 active:scale-[0.98] motion-reduce:transition-none motion-reduce:hover:scale-100"
        >
          <span className="text-2xl leading-none">+</span>
          <span className="mt-2 text-xs">New list</span>
        </button>
        {lists.map((list, i) => (
          <div
            key={list.id}
            className="prosc-enter w-[180px] shrink-0"
            style={{ animationDelay: `${Math.min(i + 1, 10) * 30}ms` }}
          >
            <ListCoverCard
              list={list}
              onOpen={onOpenList}
              onMenu={(l, x, y) => setMenu({ list: l, x, y })}
            />
          </div>
        ))}
      </ScrollRow>

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { label: "Open", onSelect: () => onOpenList(menu.list.id) },
            { label: "Rename…", onSelect: () => setEditor({ mode: "rename", list: menu.list }) },
            { label: "Delete", onSelect: () => setDeleting(menu.list) },
          ]}
        />
      )}

      {deleting && (
        <ConfirmDialog
          title={`Delete "${deleting.name}"?`}
          message={
            deleting.itemCount > 0
              ? `This permanently removes the list and its ${deleting.itemCount} ${
                  deleting.itemCount === 1 ? "item" : "items"
                }.`
              : "This permanently removes the list."
          }
          confirmLabel="Delete"
          danger
          onConfirm={() => {
            const id = deleting.id;
            setDeleting(null);
            void remove(id);
          }}
          onCancel={() => setDeleting(null)}
        />
      )}

      {editor?.mode === "create" && (
        <ListEditorDialog
          title="New list"
          submitLabel="Create"
          onSubmit={(name) => {
            setEditor(null);
            void create(name);
          }}
          onClose={() => setEditor(null)}
        />
      )}
      {editor?.mode === "rename" && (
        <ListEditorDialog
          title="Rename list"
          initialName={editor.list.name}
          submitLabel="Save"
          onSubmit={(name) => {
            const id = editor.list.id;
            setEditor(null);
            void rename(id, name);
          }}
          onClose={() => setEditor(null)}
        />
      )}
    </section>
  );
}
