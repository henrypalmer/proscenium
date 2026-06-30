import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import CanonicalBrowse from "../components/canonical/CanonicalBrowse";
import MovieDetail from "../components/vod/MovieDetail";
import type { Movie } from "../types";

/**
 * Movies browse (Milestone 40): a Cinemeta-backed **canonical** catalog (genre
 * sidebar + paged poster grid + canonical detail). Provider-centric browse
 * remains reachable: a movie search result still navigates here with a provider
 * `Movie` to open its provider detail overlay (spec §19 M40 — provider-centric
 * browse stays for un-matched VOD / search; the canonical→source picker lands
 * in slice 3).
 */
export default function Movies() {
  const location = useLocation();
  const navigate = useNavigate();
  const [navMovie, setNavMovie] = useState<Movie | null>(
    (location.state as { openMovie?: Movie } | null)?.openMovie ?? null,
  );

  useEffect(() => {
    const s = location.state as { openMovie?: Movie } | null;
    if (s?.openMovie) {
      setNavMovie(s.openMovie);
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
    </>
  );
}
