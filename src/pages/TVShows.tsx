import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import CanonicalBrowse from "../components/canonical/CanonicalBrowse";
import CanonicalDetail from "../components/canonical/CanonicalDetail";
import SeriesDetail from "../components/vod/SeriesDetail";
import type { CanonicalItem, Series } from "../types";

/**
 * TV Shows browse (Milestone 40): a Cinemeta-backed **canonical** catalog. Two
 * detail overlays open via navigation state: a provider `Series` (`openSeries`)
 * → its provider detail, and a canonical search hit (`openCanonical`, M43) → the
 * canonical detail + per-episode source picker.
 */
export default function TVShows() {
  const location = useLocation();
  const navigate = useNavigate();
  const [navSeries, setNavSeries] = useState<Series | null>(
    (location.state as { openSeries?: Series } | null)?.openSeries ?? null,
  );
  const [navCanonical, setNavCanonical] = useState<CanonicalItem | null>(
    (location.state as { openCanonical?: CanonicalItem } | null)?.openCanonical ?? null,
  );

  useEffect(() => {
    const s = location.state as
      | { openSeries?: Series; openCanonical?: CanonicalItem }
      | null;
    if (s?.openSeries) setNavSeries(s.openSeries);
    if (s?.openCanonical) setNavCanonical(s.openCanonical);
    if (s?.openSeries || s?.openCanonical) {
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.state, location.pathname, navigate]);

  return (
    <>
      <CanonicalBrowse kind="series" allLabel="All Series" emptyNoun="shows" />
      {navSeries && (
        <SeriesDetail
          providerId={navSeries.providerId}
          series={navSeries}
          onClose={() => {
            setNavSeries(null);
            navigate(-1);
          }}
        />
      )}
      {navCanonical && (
        <CanonicalDetail
          item={navCanonical}
          onClose={() => {
            setNavCanonical(null);
            navigate(-1);
          }}
        />
      )}
    </>
  );
}
