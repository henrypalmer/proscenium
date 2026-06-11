export function formatUnixDate(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds <= 0) {
    return "Never";
  }
  return new Date(seconds * 1000).toLocaleString();
}
