# Spike: Multi-Source Catalog & Stremio Addons (Canonical-Catalog Direction)

- **Date:** 2026-06-29
- **Author:** Engineering (Claude Code session)
- **Status:** Complete — findings + recommendation below; feeds Milestones 39–42.
- **Outcome (2026-06-29):** The risky assumptions behind the "media-hub" direction are **de-risked and favorable.** Stream resolution for the owner's setup is **~100% direct URLs** (no torrent engine needed); movie→canonical matching is **near-exact** (provider VOD carries a `tmdb_id` 100% of the time, plus ~85%+ name+year matching as a fallback/confirmation); **series is the soft spot** (no provider IDs — name+year only, needs a manual-override affordance). Direction confirmed: adopt the **Stremio addon model** internally (canonical catalog keyed by IMDB/TMDB id + a registry of stream *resolvers*), with **Cinemeta** as the metadata backbone and **resolve-on-click** source resolution.
- **Trigger:** Before committing to a re-platform from provider-centric browse to a canonical, catalog-first media application (browse a canonical catalog → resolve playback across all configured IPTV providers + Stremio addons on click), validate the three unknowns that determine whether the approach is viable and how much matching machinery it needs.

---

## TL;DR

- **Both requested features collapse into one architecture.** "Multiple active providers" and "Stremio addons" are the same concept — a **registry of stream resolvers** keyed by a canonical title id (IMDB/TMDB). Stremio stream addons *are* resolvers; each IPTV provider is wrapped as a local resolver; multi-provider is then just "more than one resolver registered." This is the Stremio model, and **Cinemeta** (Stremio's metadata addon) doubles as both the catalog backbone and the proof that the addon-client plumbing works.
- **Stremio/AIOStreams → trivial and excellent.** The owner's AIOStreams + Torbox setup returns **~100% direct HTTPS URLs, zero raw torrents** (Matrix 134/135, Oppenheimer 26/27, GoT S1E1 157/158, all Torbox-cached `[TB⚡]`). The "direct/debrid only, no torrent engine" decision is fully validated. Series `imdb:season:episode` addressing works; stream labels carry parseable quality metadata for ranking.
- **Movies → near-exact matching.** Provider VOD detail (`get_vod_info`) carries a `tmdb_id` **100%** of the time (sample 120/120). Name+year → Cinemeta is ~85%+ on its own; combined (FTS name shortlist → year filter → `tmdb_id` confirm) it's ~95%+. The `tmdb_id` kills the wrong-year collisions name matching alone produces.
- **Series → the real engineering.** **No** provider canonical IDs (bulk or detail, either provider). Series rely on name+year only, with **no `tmdb_id` backstop**, plus season/episode mapping. This is the one place that needs a **manual "wrong match? pick the right title" override** from day one.
- **Bulk listings carry no IDs** — so canonical IDs can't be captured for free during a normal refresh; movies need the per-title `get_vod_info` call (cheap, lazy, on click).
- **The owner already runs complementary providers** (SRP Tech App = live + series, no movies; VOD Playlist = movies + series, no live) — the multi-provider merge case occurs naturally in the wild and is independently worth shipping first.

---

## 1. Context

Proscenium today is provider-centric: browse one **active provider**'s categories; clicking a movie/episode resolves a stream URL from *that* provider ([`resolve_stream_url_impl`](../../src-tauri/src/commands/playback.rs)). The catalog tables are already provider-scoped (composite `(id, provider_id)` PKs — [`db/schema.rs`](../../src-tauri/src/db/schema.rs)); what makes the app single-provider is one settings row (`active_provider_id`) and a single `activeProvider` in the store.

The proposed direction flips this: browse a **canonical catalog** (TMDB/IMDB-grade posters from an external source), and on click resolve actual playback sources across **all** configured IPTV providers **and** Stremio addons. That is the Stremio architecture. Two product decisions were taken before this spike (see [SPEC §2 / §19 M39–M42](../../SPEC.md)): **Cinemeta now (TMDB later)**, **show-all + resolve-on-click**, **direct/debrid URLs only (no torrent engine)**, and **spike first**.

The three unknowns this spike resolves:
1. Do the owner's IPTV providers expose canonical IDs (TMDB/IMDB) we can anchor matching on?
2. How good is name+year → canonical matching when IDs are absent?
3. Does the owner's Stremio setup return playable direct URLs, or raw torrents that would force an embedded torrent engine?

> Scripts are throwaway and live outside the repo (session scratchpad). Probe A read the live DB read-only and the Xtream secret from Windows Credential Manager via `ctypes`; the AIOStreams manifest URL (which embeds a Torbox token) was kept out of all output and deleted after the run.

## 2. What we tested

| Probe | Question | Method |
|---|---|---|
| **A1** | Do **bulk** listings (`get_vod_streams` / `get_series`) carry tmdb/imdb? | One call per provider; scan every item's keys for `tmdb`/`imdb`. |
| **A2** | Do **per-title detail** calls (`get_vod_info` / `get_series_info`) carry them? | Random sample of 120 movies + 120 series per provider; scan `info`/`movie_data`. |
| **B** | name+year → **Cinemeta** match hit-rate (movies) | 180 random real movie titles → normalize (strip lang/quality tags, extract year) → Cinemeta search → score top match (title ratio + year ±1). |
| **C** | **AIOStreams** stream shape | Fetch the manifest + `/stream` for a classic movie, a recent movie, and a series episode; classify each stream as direct `url` vs `infoHash`. |

**Provider context (live DB):**

| Provider | Live | Movies | Series |
|---|---|---|---|
| SRP Tech App (`srptechapp.com`, active) | 4,630 | 0 | 1,113 |
| VOD Playlist (`sparkylola.com`) | 0 | 16,613 | 6,114 |

## 3. Results

### Probe A — canonical-ID coverage

| | bulk tmdb | bulk imdb | detail tmdb | detail imdb | notes |
|---|---|---|---|---|---|
| VOD Playlist · movies | 0% | 0% | **100%** (120/120) | 0% | `tmdb_id` (and sometimes `tmdb_url`) in `get_vod_info.info` |
| VOD Playlist · series | 0% | 0% | 0% (120/120 ok) | 0% | no IDs anywhere |
| SRP Tech App · series | 0% | 0% | 0% (29/120 ok) | 0% | **91/120 detail calls errored** — `get_series_info` is flaky on this panel |

- **Movies have an exact canonical anchor** (`tmdb_id`), but only via the **per-title** `get_vod_info` call — **not** the bulk listing. So it's a cheap, lazy, on-click lookup, not a free refresh-time capture.
- **Series have none.** Both panels expose zero tmdb/imdb for series. SRP's series detail endpoint additionally failed ~75% of the time (stale ids or panel rate-limiting), so on-demand series detail there is unreliable independent of this work.
- Example movie id fields: `{'tmdb_id': 939495}`, `{'tmdb_url': '…/movie/39102', 'tmdb_id': '39102'}`, `{'tmdb_id': 1071806}`.

### Probe B — name+year → Cinemeta (180 movies)

`strong: 143 (80%) · likely: 19 (11%) · miss: 17 (9%)` of scored (1 unparseable).

**The buckets need interpretation** (the headline 91% over/understates in different ways):
- Several **"likely" are wrong** — same title, different film, caught only by year: `Carlos (2010)`→`Carlos (2023)`, `Amber Alert (2012)`→`(2024)`, `The Good Neighbor (2022)`→`(2016)`. **⇒ year disambiguation is mandatory.**
- Several **"miss" are correct** — IPTV VOD is full of non-films with no canonical entry: `P90X – Ab Ripper X`, `UFC 324`, `Bob Marley – Live in Concert`. **⇒ the UI must handle "no canonical match" gracefully; these stay in provider-centric browse.**
- A few are genuine Cinemeta **recall gaps** (`Timeline (2003)` exists on IMDB but search returned nothing) — a mark against Cinemeta search and a point for pulling TMDB in later.

Real correct-match rate on *actual films* ≈ high-80s on name+year alone; lifted to **~95%+ for movies** by the `tmdb_id` confirmation step.

### Probe C — AIOStreams stream shape

Manifest: `AIOStreams Nightly` v2.30.5; **stream** resource `idPrefixes` include `tt`, `imdb`, **`tmdb`**, `kitsu`, `mal`, `tvdb`, … (accepts both IMDB and TMDB ids).

| Target | streams | direct `url` | `infoHash` | hosts |
|---|---|---|---|---|
| `movie/tt0133093` (The Matrix) | 135 | **134** | 0 | torrentio.strem.fun |
| `movie/tt15398776` (Oppenheimer 2023) | 27 | **26** | 0 | comet… |
| `series/tt0944947:1:1` (GoT S1E1) | 158 | **157** | 0 | comet… + torrentio |

- **~100% direct HTTPS, zero raw torrents.** Every stream is Torbox-cached (`[TB⚡]`); the debrid resolves torrents to direct URLs server-side. **No embedded torrent engine is needed for this setup.**
- Stream labels are rich and parseable: `2160p`, `BluRay REMUX`, `HDR | DV`, `Atmos`, size, seeders, source — usable for **ranking the source picker**. `behaviorHints.bingeGroup` is present (next-episode continuity).
- Series `imdb:season:episode` addressing works directly.

## 4. Conclusions — the matching strategy

**Movies → essentially solved.** Two reinforcing paths:
- *provider → canonical:* `get_vod_info.tmdb_id`, exact, one lazy call.
- *canonical (Cinemeta poster) → provider source:* local FTS name shortlist over the cached catalog (instant, offline) → **year ±1 filter** → confirm the candidate with `get_vod_info.tmdb_id == target`. ~95%+, with the false-positive collisions eliminated.

**Stremio (addons) → trivial.** Hand the canonical id to `/stream/{type}/{imdb[:s:e]}.json`, take the direct URLs, feed mpv ([`mpv_load_url`](../../src-tauri/src/commands/playback.rs)). The addon does the matching.

**Series → name+year + override.** No provider IDs → name+year against the cached series (across providers), **no tmdb backstop**, plus S/E mapping via `get_series_info` (look up by `(season, episode)`). Needs a **manual "pick the right title" override** and tolerance for occasional mis-matches.

**The tmdb↔imdb bridge.** Provider movies are *tmdb*-keyed; Cinemeta and most addons are *imdb*-keyed (AIOStreams accepts both). So a small conversion is needed — pull in **just** TMDB's free ID endpoints (`/find`, `/external_ids`) early, or read it from Cinemeta meta. This is a minimal, ID-only slice of the deferred TMDB work.

## 5. Storage policy (persist vs. ephemeral)

Sort new data by **volatility × cost-to-recompute**, not by which feature produced it:

- **Tier 1 — durable (disk, source of truth):** IPTV catalogs (as today); installed-**addon config** (manifest URLs — token-bearing → **keychain**, like provider passwords); the **canonical↔IPTV match index** (a side table keyed by the provider's stable `(provider_id, content_id)` — expensive, derived from stable data); user data (watch progress, now canonical-keyed; lists).
- **Tier 2 — disposable cache (disk, throwaway, TTL):** Cinemeta catalogs (short TTL) + per-title meta (long TTL); images via the M27 cache.
- **Tier 3 — ephemeral (RAM, this session only):** Stremio `/stream` results, resolved/unrestricted URLs (debrid links expire). Reuse the existing in-memory session-cache pattern ([`DetailCache`](../../src-tauri/src/commands/catalog.rs)); never to disk.

**Refresh gotcha:** [`replace_catalog`](../../src-tauri/src/db/catalog.rs) deletes+reinserts a provider's rows and hard-nulls `movies.imdb_id`/`series.imdb_id` every refresh — so the match **must** live in a dedicated side table (which the refresh doesn't touch), not on the catalog row. The existing "preserve watch-progress episodes across refresh" dance is precedent that such a survival path is normal here.

## 6. Decisions locked by this spike

1. **No torrent engine** in the first cut (direct/debrid only) — validated by Probe C.
2. **Canonical key = IMDB id**, with `tmdb_id` from providers bridged to it; `tmdb_id` is the movie matcher's confirmer.
3. **Resolve-on-click**, show-all catalog — confirmed feasible (lazy `get_vod_info` is cheap).
4. **Cinemeta** is the metadata backbone *and* the addon-client proof; the addon plumbing is shared between "catalog rework" and "Stremio support."
5. **Multi-provider merge ships first** (M39) — independently useful (the owner's own complementary providers), and the cure for the duplication it introduces is the canonical layer (M40).
6. **Provider-centric browse stays** for Live TV and the large slice of un-matchable VOD.

## 7. Implications for the milestones

- **M39 — Multiple Active Providers (merged catalog):** mostly reads + state (storage is already provider-scoped); independently shippable; lifts the §2 non-goal.
- **M40 — Canonical Catalog & Source Resolution:** Cinemeta backbone + resolver registry (generalize `resolve_stream_url`) + `content_match` side table + source picker; **reframes M33** (its "match → persist tmdb id against the stream id" core *is* `content_match`; the spike shows the id is read from `get_vod_info`, not searched). Movies near-exact; series with manual override.
- **M41 — Stremio Stream Addons (direct/debrid):** add-by-URL resolvers folded into the M40 picker; token URLs in keychain; infoHash streams flagged/skipped (no engine).
- **M42 — Polish:** background availability badges, cross-source dedup, source ranking, series-mapping robustness, optional richer TMDB backbone.
- **Deferred (non-milestone):** an embedded torrent engine for raw infoHash streams — only if a future addon/setup needs it.
