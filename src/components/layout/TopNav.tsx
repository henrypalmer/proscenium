import { useLayoutEffect, useRef, useState } from "react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { useCatalogStore } from "../../store/catalogStore";
import { useProviderStore } from "../../store/providerStore";
import { useSearchStore } from "../../store/searchStore";
import ContextMenu from "../common/ContextMenu";

/** Primary navigation (spec §9): a floating, horizontally-centered pill pinned
 * to the top of the content area — Home · Live TV · Movies · Series ·
 * Settings, in that fixed order. Replaces the former left sidebar. */
const NAV_ITEMS = [
  {
    to: "/",
    label: "Home",
    end: true,
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
        <path d="M3 10.5 12 3l9 7.5" />
        <path d="M5 9.5V21h14V9.5" />
      </svg>
    ),
  },
  {
    to: "/live",
    label: "Live TV",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
        <rect x="3" y="6" width="18" height="13" rx="2" />
        <path d="m8 2 4 4 4-4" />
      </svg>
    ),
  },
  {
    to: "/movies",
    label: "Movies",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <path d="M3 9h18M7 4v5M12 4v5M17 4v5" />
      </svg>
    ),
  },
  {
    to: "/shows",
    label: "Series",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
        <rect x="3" y="5" width="18" height="13" rx="2" />
        <path d="M9 21h6M10 9.5l4 2-4 2v-4Z" />
      </svg>
    ),
  },
  {
    to: "/settings",
    label: "Settings",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h.01a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h.01a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.01a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z" />
      </svg>
    ),
  },
];

/** Shared bubble styling: the same background as the nav pill, but each control
 * is its own disjointed rounded container (spec §9 / user request). */
const BUBBLE_BASE =
  "rounded-full border border-zinc-800 bg-zinc-900/90 text-zinc-300 shadow-xl backdrop-blur transition-colors";
/** Icon-only action bubbles (search / refresh). */
const BUBBLE_CLASS = `flex items-center justify-center p-2.5 ${BUBBLE_BASE} hover:text-white disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:text-zinc-300`;
/** Provider label bubble — same look, sized for text, name truncates. */
const PROVIDER_BUBBLE_CLASS = `pointer-events-auto flex min-w-0 max-w-[14rem] items-center gap-2 px-4 py-2.5 text-sm font-medium ${BUBBLE_BASE} hover:text-white`;

/** Refresh progress ring geometry (drawn over the 40px refresh bubble). */
const RING_R = 18;
const RING_CIRC = 2 * Math.PI * RING_R;

export default function TopNav() {
  const navigate = useNavigate();
  const providers = useProviderStore((s) => s.providers);
  const enabledProviders = useCatalogStore((s) => s.enabledProviders);
  const providerIds = useCatalogStore((s) => s.providerIds);
  const refreshing = useCatalogStore((s) => s.refreshing);
  const stage = useCatalogStore((s) => s.stage);
  const progress = useCatalogStore((s) => s.progress);
  const refresh = useCatalogStore((s) => s.refresh);
  const toggleProvider = useCatalogStore((s) => s.toggleProvider);
  const openSearch = useSearchStore((s) => s.setOpen);

  // Pill label: the single enabled provider's name, or "N providers" when
  // several are enabled (Milestone 39).
  const pillLabel =
    enabledProviders.length === 0
      ? (providers[0]?.name ?? "No provider")
      : enabledProviders.length === 1
        ? enabledProviders[0].name
        : `${enabledProviders.length} providers`;

  // Provider switcher (Milestone 39): a dropdown listing every saved provider,
  // each checked when enabled. Selecting one toggles its membership in the
  // merged set; the user stays on the current section (the page remounts via the
  // provider-set key in App.tsx).
  const [switcherAt, setSwitcherAt] = useState<{ x: number; y: number } | null>(
    null,
  );

  // The white selection pill is a single floating element that physically
  // slides between items (CSS transition on its measured geometry). It is
  // painted *over* the labels with `mix-blend-mode: difference`, so the labels
  // never move and stay visible — each one only inverts (reads dark) where the
  // white pill actually covers it, continuously as the pill slides past.
  const location = useLocation();
  const navRef = useRef<HTMLElement>(null);
  const linkRefs = useRef<Array<HTMLAnchorElement | null>>([]);
  const [pill, setPill] = useState<
    { left: number; top: number; width: number; height: number } | null
  >(null);

  const activeIndex = NAV_ITEMS.findIndex((item) =>
    item.end
      ? location.pathname === item.to
      : location.pathname === item.to ||
        location.pathname.startsWith(`${item.to}/`),
  );

  // Measure the active item and position the pill over it (re-measuring on
  // resize / font load). Runs before paint so the pill never flashes in.
  useLayoutEffect(() => {
    const measure = () => {
      const link = linkRefs.current[activeIndex];
      if (!link) {
        setPill(null);
        return;
      }
      setPill({
        left: link.offsetLeft,
        top: link.offsetTop,
        width: link.offsetWidth,
        height: link.offsetHeight,
      });
    };
    measure();
    const nav = navRef.current;
    const ro = nav ? new ResizeObserver(measure) : null;
    if (nav && ro) ro.observe(nav);
    return () => ro?.disconnect();
  }, [activeIndex]);

  const linkClass =
    "relative flex shrink-0 items-center gap-2 whitespace-nowrap rounded-full px-3.5 py-1.5 text-sm font-medium text-white transition-colors hover:bg-zinc-800";

  return (
    <>
    <div className="pointer-events-none absolute inset-x-0 top-5 z-30 flex items-center justify-center gap-2 px-4">
      {providers.length > 0 && (
        <button
          onClick={(e) => {
            const r = e.currentTarget.getBoundingClientRect();
            setSwitcherAt({ x: r.left, y: r.bottom + 6 });
          }}
          title="Providers — enable or disable which catalogs are merged"
          aria-label="Providers. Enable or disable which catalogs are merged"
          aria-haspopup="menu"
          data-testid="provider-pill"
          className={PROVIDER_BUBBLE_CLASS}
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            className="h-4 w-4 shrink-0 text-zinc-400"
          >
            <path d="M4.9 19.1a10 10 0 0 1 0-14.2M19.1 4.9a10 10 0 0 1 0 14.2M7.8 16.2a6 6 0 0 1 0-8.4M16.2 7.8a6 6 0 0 1 0 8.4" />
            <circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none" />
          </svg>
          <span className="truncate">{pillLabel}</span>
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            aria-hidden
            className="h-3.5 w-3.5 shrink-0 text-zinc-500"
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </button>
      )}
      <nav
        ref={navRef}
        data-testid="top-nav"
        className="pointer-events-auto relative isolate flex shrink-0 items-center gap-1 rounded-full border border-zinc-800 bg-zinc-900/90 p-1 shadow-xl backdrop-blur"
      >
        {NAV_ITEMS.map((item, i) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.end}
            ref={(el) => {
              linkRefs.current[i] = el;
            }}
            data-testid={`nav-${item.label.replace(/\s+/g, "-").toLowerCase()}`}
            className={linkClass}
          >
            {item.icon}
            <span>{item.label}</span>
          </NavLink>
        ))}
        {/* Floating selection pill: painted over the labels with a difference
            blend so the label under it reads dark while every other label stays
            light and in place. Slides between items via the geometry transition. */}
        {pill && (
          <span
            aria-hidden
            data-testid="nav-pill"
            className="pointer-events-none absolute rounded-full bg-white mix-blend-difference transition-[left,top,width,height] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none"
            style={{ left: pill.left, top: pill.top, width: pill.width, height: pill.height }}
          />
        )}
      </nav>

      {/* Search + Refresh: directly beside the pill, each its own bubble (icons only). */}
      <div className="pointer-events-auto flex shrink-0 items-center gap-2">
        <button
          onClick={() => openSearch(true)}
          title="Search (Ctrl+F)"
          aria-label="Search"
          data-testid="search-trigger"
          className={BUBBLE_CLASS}
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            className="h-5 w-5"
          >
            <circle cx="11" cy="11" r="7" />
            <path d="m21 21-4.3-4.3" />
          </svg>
        </button>
        <div className="relative">
          {refreshing && (
            <svg
              viewBox="0 0 40 40"
              aria-hidden="true"
              className="pointer-events-none absolute inset-0 h-full w-full -rotate-90"
            >
              <circle
                cx="20"
                cy="20"
                r={RING_R}
                fill="none"
                stroke="currentColor"
                strokeWidth="2.5"
                className="text-zinc-700"
              />
              <circle
                cx="20"
                cy="20"
                r={RING_R}
                fill="none"
                stroke="currentColor"
                strokeWidth="2.5"
                strokeLinecap="round"
                className="text-emerald-400 transition-[stroke-dashoffset] duration-300"
                strokeDasharray={RING_CIRC}
                strokeDashoffset={RING_CIRC * (1 - Math.max(0.04, progress))}
              />
            </svg>
          )}
          <button
            onClick={() => void refresh()}
            disabled={enabledProviders.length === 0 || refreshing}
            title={refreshing ? (stage ?? "Refreshing…") : "Refresh catalog"}
            aria-label="Refresh catalog"
            data-testid="refresh-trigger"
            className={BUBBLE_CLASS}
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.8"
              className={`h-5 w-5 ${refreshing ? "animate-spin" : ""}`}
            >
              <path d="M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6" />
            </svg>
          </button>
        </div>
      </div>
    </div>
    {/* Rendered outside the pointer-events-none container so it's clickable. */}
    {switcherAt && (
      <ContextMenu
        x={switcherAt.x}
        y={switcherAt.y}
        onClose={() => setSwitcherAt(null)}
        items={[
          ...providers.map((p) => ({
            label: p.name,
            active: providerIds.includes(p.id),
            onSelect: () => void toggleProvider(p.id),
          })),
          { label: "Manage in Settings…", onSelect: () => navigate("/settings") },
        ]}
      />
    )}
    </>
  );
}
