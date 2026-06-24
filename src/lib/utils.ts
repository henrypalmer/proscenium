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

/**
 * Display-only cleanup of a provider episode title (spec §5.4, M20). Xtream
 * panels routinely embed the series name and/or an SxxEyy tag in the episode
 * `title` (e.g. "Breaking Code S01E02 — Cat's in the Bag"), which is redundant
 * inside the series-detail episode list. Strip a leading series-name, a
 * season/episode tag (S01E02, 1x02, E02, "Episode 2"), and any separators,
 * returning the clean episode name — or "Episode N" when nothing meaningful is
 * left (so a row is never blank). The provider data itself is never mutated.
 */
export function cleanEpisodeTitle(
  seriesName: string,
  episode: number,
  title: string,
): string {
  const fallback = `Episode ${episode}`;
  let s = (title ?? "").trim();
  if (!s) return fallback;

  const stripLeadingSeparators = (v: string) =>
    v.replace(/^[\s\-–—:.|·•>]+/, "").trim();

  // Drop a leading series-name prefix (case-insensitive).
  const name = seriesName.trim();
  if (name && s.toLowerCase().startsWith(name.toLowerCase())) {
    s = stripLeadingSeparators(s.slice(name.length));
  }

  // Drop a leading season/episode tag: S01E02, 1x02, E02, "Episode 2", "Ep 2".
  s = stripLeadingSeparators(
    s.replace(
      /^(?:s\s*\d{1,4}\s*[\s._-]*e\s*\d{1,4}|\d{1,4}\s*x\s*\d{1,4}|(?:episode|ep|e)\s*\.?\s*\d{1,4})/i,
      "",
    ),
  );

  return s.length > 0 ? s : fallback;
}

/** Clock-style position: 95 → "1:35"; 3725 → "1:02:05". */
export function formatTimestamp(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}
