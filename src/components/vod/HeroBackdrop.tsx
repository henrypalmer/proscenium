import { useState } from "react";

/**
 * Full-bleed hero backdrop for the movie/series detail view (spec §5.4,
 * Milestone 18). Uses the provider's real backdrop when present; otherwise
 * derives a blurred, darkened treatment from the poster so the hero is never
 * flat black. Purely presentational — the title, metadata, and poster are
 * layered over it by the caller. The image is static (no animation) so it
 * respects the §9/§10 motion and performance guardrails.
 */
export default function HeroBackdrop({
  backdropUrl,
  posterUrl,
}: {
  backdropUrl: string | null;
  posterUrl: string | null;
}) {
  // Prefer the real backdrop; fall back to the poster (blurred + zoomed so the
  // 2:3 art fills the wide band without showing its edges).
  const usingPoster = !backdropUrl;
  const src = backdropUrl ?? posterUrl;
  const [loaded, setLoaded] = useState(false);

  return (
    <div
      aria-hidden
      data-testid="detail-hero"
      data-hero-source={backdropUrl ? "backdrop" : posterUrl ? "poster" : "none"}
      className="pointer-events-none absolute inset-x-0 top-0 h-[420px] overflow-hidden"
    >
      {src && (
        <img
          src={src}
          alt=""
          decoding="async"
          onLoad={() => setLoaded(true)}
          onError={() => setLoaded(false)}
          className={`h-full w-full object-cover object-top transition-opacity duration-300 ${
            loaded ? "opacity-100" : "opacity-0"
          } ${usingPoster ? "scale-125 blur-2xl" : ""}`}
        />
      )}
      {/* Vertical scrim: darken the art and fade it into the page background so
          content below the hero reads on solid bg-zinc-950. */}
      <div className="absolute inset-0 bg-gradient-to-b from-zinc-950/30 via-zinc-950/60 to-zinc-950" />
      {/* Horizontal scrim: anchor the title/metadata side for legibility. */}
      <div className="absolute inset-0 bg-gradient-to-r from-zinc-950/80 via-zinc-950/30 to-transparent" />
    </div>
  );
}
