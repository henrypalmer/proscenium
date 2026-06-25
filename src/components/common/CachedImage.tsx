import { useEffect, useRef, useState } from "react";
import { resolveArtSrc } from "../../lib/imageCache";

interface CachedImageProps {
  /** Remote art URL, or null for "no image" (renders nothing). */
  url: string | null;
  /** Classes for the <img> (positioning, object-fit, filters). The opacity
   * fade-in is applied internally. */
  className?: string;
  /** Load eagerly instead of lazily (e.g. an above-the-fold hero). */
  eager?: boolean;
}

/**
 * Catalog art routed through the on-disk image cache (spec §5.7, Milestone 27).
 * On a cache hit it loads the local file via the asset protocol (no network);
 * on a miss it shows the remote URL and caches it in the background for next
 * time. If a cached file fails to load it falls back to the remote URL once.
 * Renders nothing until a src resolves, so the caller's placeholder shows
 * through with no layout shift.
 */
export default function CachedImage({ url, className = "", eager }: CachedImageProps) {
  const [src, setSrc] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const triedRemote = useRef(false);

  useEffect(() => {
    setSrc(null);
    setLoaded(false);
    triedRemote.current = false;
    if (!url) return;
    let cancelled = false;
    void resolveArtSrc(url).then((resolved) => {
      if (!cancelled) setSrc(resolved);
    });
    return () => {
      cancelled = true;
    };
  }, [url]);

  if (!url || !src) return null;
  return (
    <img
      src={src}
      alt=""
      loading={eager ? undefined : "lazy"}
      decoding="async"
      onLoad={() => setLoaded(true)}
      onError={() => {
        // A cached file that won't load → retry the remote URL once.
        if (!triedRemote.current && src !== url) {
          triedRemote.current = true;
          setSrc(url);
        }
      }}
      className={`${className} transition-opacity duration-150 ${
        loaded ? "opacity-100" : "opacity-0"
      }`}
    />
  );
}
