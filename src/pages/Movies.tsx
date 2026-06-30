import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import CanonicalBrowse from "../components/canonical/CanonicalBrowse";
import CanonicalDetail from "../components/canonical/CanonicalDetail";
import MovieDetail from "../components/vod/MovieDetail";
import type { CanonicalItem, Movie } from "../types";

/**
 * Movies browse (Milestone 40): a Cinemeta-backed **canonical** catalog (genre
 * sidebar + paged poster grid + canonical detail). Two detail overlays open via
 * navigation state: a provider `Movie` (`openMovie`) → its provider detail, and
 * a canonical search hit (`openCanonical`, M43) → the canonical detail + source
 * picker, so a search result reaches the same multi-source flow as Browse.
 */
export default function Movies() {
  const location = useLocation();
  const navigate = useNavigate();
  const [navMovie, setNavMovie] = useState<Movie | null>(
    (location.state as { openMovie?: Movie } | null)?.openMovie ?? null,
  );
  const [navCanonical, setNavCanonical] = useState<CanonicalItem | null>(
    (location.state as { openCanonical?: CanonicalItem } | null)?.openCanonical ?? null,
  );

  useEffect(() => {
    const s = location.state as
      | { openMovie?: Movie; openCanonical?: CanonicalItem }
      | null;
    if (s?.openMovie) setNavMovie(s.openMovie);
    if (s?.openCanonical) setNavCanonical(s.openCanonical);
    if (s?.openMovie || s?.openCanonical) {
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.state, location.pathname, navigate]);

  return (
    <>
      <CanonicalBrowse kind="movie" allLabel="All Movies" emptyNoun="movies" />
      {navMovie && (
        <MovieDetail
          providerId={navMovie.providerId}
          movie={navMovie}
          onClose={() => {
            setNavMovie(null);
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
