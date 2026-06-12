interface PlaceholderProps {
  label: string;
}

/** Styled fallback shown when no logo/poster image is available (spec §18). */
export default function Placeholder({ label }: PlaceholderProps) {
  const initial = label.trim().charAt(0).toUpperCase() || "?";
  return (
    <div className="flex h-full w-full items-center justify-center bg-zinc-800 text-sm font-semibold text-zinc-500">
      {initial}
    </div>
  );
}
