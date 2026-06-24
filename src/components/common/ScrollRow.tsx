import { useEffect, useRef, useState, type ReactNode } from "react";

interface ScrollRowProps {
  children: ReactNode;
  /** Extra classes for the scroll container. */
  className?: string;
}

/**
 * A horizontally-scrollable strip with hover-reveal left/right chevron buttons
 * (spec §9 / Milestone 26), standardized across the Home rows and the per-genre
 * rows. The chevrons appear on hover only when there is overflow in that
 * direction and scroll roughly one viewport per click. The scroll container
 * keeps the `-mx-2 … py-2` breathing room so hovered/scaled cards aren't clipped.
 */
export default function ScrollRow({ children, className = "" }: ScrollRowProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);

  const update = () => {
    const el = ref.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 4);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 4);
  };

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  // Re-evaluate after every render so newly-loaded items toggle the chevrons
  // (ResizeObserver doesn't fire on scrollWidth-only changes). setState bails
  // out when the value is unchanged, so this settles in one extra pass.
  useEffect(update);

  const scrollBy = (dir: number) => {
    const el = ref.current;
    if (!el) return;
    const behavior = window.matchMedia("(prefers-reduced-motion: reduce)").matches
      ? "auto"
      : "smooth";
    el.scrollBy({ left: dir * el.clientWidth * 0.8, behavior });
  };

  const chevronBase =
    "absolute top-1/2 z-20 hidden h-9 w-9 -translate-y-1/2 items-center justify-center rounded-full border border-zinc-700 bg-zinc-900/90 text-zinc-100 shadow-lg backdrop-blur transition hover:bg-zinc-800 hover:text-white group-hover/scroll:flex";

  return (
    <div className="group/scroll relative">
      <div
        ref={ref}
        onScroll={update}
        className={`-mx-2 flex gap-4 overflow-x-auto px-2 py-2 ${className}`}
      >
        {children}
      </div>
      {canLeft && (
        <button
          type="button"
          aria-label="Scroll left"
          data-testid="scroll-left"
          onClick={() => scrollBy(-1)}
          className={`${chevronBase} left-1`}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" className="h-5 w-5">
            <path d="M15 6l-6 6 6 6" />
          </svg>
        </button>
      )}
      {canRight && (
        <button
          type="button"
          aria-label="Scroll right"
          data-testid="scroll-right"
          onClick={() => scrollBy(1)}
          className={`${chevronBase} right-1`}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" className="h-5 w-5">
            <path d="M9 6l6 6-6 6" />
          </svg>
        </button>
      )}
    </div>
  );
}
