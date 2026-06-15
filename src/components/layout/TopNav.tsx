import { NavLink } from "react-router-dom";

/** Primary navigation (spec §9): a floating, horizontally-centered pill pinned
 * to the top of the content area — Home · Live TV · Movies · TV Shows ·
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
    label: "TV Shows",
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

export default function TopNav() {
  const linkClass = ({ isActive }: { isActive: boolean }) =>
    `flex items-center gap-2 rounded-full px-3.5 py-1.5 text-sm font-medium transition-colors ${
      isActive
        ? "bg-zinc-100 text-zinc-900"
        : "text-zinc-300 hover:bg-zinc-800 hover:text-white"
    }`;

  return (
    <nav
      data-testid="top-nav"
      className="pointer-events-auto absolute left-1/2 top-3 z-30 -translate-x-1/2"
    >
      <div className="flex items-center gap-1 rounded-full border border-zinc-800 bg-zinc-900/90 p-1 shadow-xl backdrop-blur">
        {NAV_ITEMS.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.end}
            data-testid={`nav-${item.label.replace(/\s+/g, "-").toLowerCase()}`}
            className={linkClass}
          >
            {item.icon}
            <span>{item.label}</span>
          </NavLink>
        ))}
      </div>
    </nav>
  );
}
