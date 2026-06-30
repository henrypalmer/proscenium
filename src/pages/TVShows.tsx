import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import CanonicalBrowse from "../components/canonical/CanonicalBrowse";
import SeriesDetail from "../components/vod/SeriesDetail";
import type { Series } from "../types";

/**
 * TV Shows browse (Milestone 40): a Cinemeta-backed **canonical** catalog. As in
 * Movies, a series search result still navigates here with a provider `Series`
 * to open its provider detail overlay (provider-centric browse stays for
 * un-matched VOD / search; the canonical→source picker lands in slice 3).
 */
export default function TVShows() {
  const location = useLocation();
  const navigate = useNavigate();
  const [navSeries, setNavSeries] = useState<Series | null>(
    (location.state as { openSeries?: Series } | null)?.openSeries ?? null,
  );

  useEffect(() => {
    const s = location.state as { openSeries?: Series } | null;
    if (s?.openSeries) {
      setNavSeries(s.openSeries);
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
    </>
  );
}
