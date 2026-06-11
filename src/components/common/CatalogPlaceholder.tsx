interface CatalogPlaceholderProps {
  section: string;
}

// Stand-in empty state until catalog refresh ships in Milestone 2.
export default function CatalogPlaceholder({
  section,
}: CatalogPlaceholderProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="h-10 w-10 text-zinc-700">
        <rect x="3" y="5" width="18" height="14" rx="2" />
        <path d="M10 9.5l4.5 2.5-4.5 2.5v-5Z" />
      </svg>
      <p className="text-sm font-medium text-zinc-400">No catalog yet</p>
      <p className="max-w-xs text-xs text-zinc-600">
        {section} content will appear here after the first catalog refresh.
      </p>
    </div>
  );
}
