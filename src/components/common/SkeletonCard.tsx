/**
 * Loading placeholder matching ChannelCard dimensions exactly (h-14, same
 * paddings) so resolving content causes no layout shift.
 */
export default function SkeletonCard() {
  return (
    <div className="flex h-14 w-full items-center gap-3 border-b border-zinc-900 px-4">
      <div className="h-10 w-10 shrink-0 animate-pulse rounded-md bg-zinc-800" />
      <div className="h-3 w-48 animate-pulse rounded bg-zinc-800" />
    </div>
  );
}
