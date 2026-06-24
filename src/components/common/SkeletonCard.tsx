/**
 * Loading placeholder matching ChannelCard dimensions exactly (same height and
 * paddings per density) so resolving content causes no layout shift.
 */
export default function SkeletonCard({ compact = false }: { compact?: boolean }) {
  return (
    <div
      className={`flex w-full items-center gap-3 border-b border-zinc-900 px-4 ${
        compact ? "h-11" : "h-14"
      }`}
    >
      <div
        className={`shrink-0 animate-pulse rounded-md bg-zinc-800 ${
          compact ? "h-8 w-8" : "h-10 w-10"
        }`}
      />
      <div className="h-3 w-48 animate-pulse rounded bg-zinc-800" />
    </div>
  );
}
