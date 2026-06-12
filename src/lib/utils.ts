export function formatUnixDate(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds <= 0) {
    return "Never";
  }
  return new Date(seconds * 1000).toLocaleString();
}

/** 10260 → "2h 51m"; 1260 → "21m". */
export function formatDuration(seconds: number): string {
  const minutes = Math.round(seconds / 60);
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}
