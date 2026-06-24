# Prompt Used
```text
Your job for this session is to be a thorough end user tester of Proscenium. Your goal is to take notes on positives, negatives, and areas for improvement as you navigate and use Proscenium. You are a professional QA lead. Click through all of the screens, test all of the flows between screens, taking note of things that work well and things that feel awkward. The notes you produce should be formatted such that they can be passed off to an engineer who can act on the points made. Do not use faked data. There is already an existing provider setup, SRP Tech App, so please use it for your testing.

The app is available at 'C:\Users\henry\Documents\workspace\proscenium\src-tauri\target\release\proscenium.exe'
```

# Proscenium — End-User QA Notes

**Tester:** QA lead (manual exploratory session)
**Build under test:** Local release build `src-tauri/target/release/proscenium.exe`
**Provider:** SRP Tech App (existing, real data)
**Date:** 2026-06-24
**Platform:** Windows 11

Legend: ✅ Positive · 🐞 Bug/defect · ⚠️ Awkward/UX friction · 💡 Improvement idea

---

## Executive summary

Proscenium is in strong shape as an end-user product. The information architecture (Home with Keep Watching + My Lists + Popular rows, plus Live TV / Movies / Series / Search / Settings) is coherent and consistent, real provider content loads quickly, and the core jobs — browse, search, play, resume — all work on real streams. Standouts: the Netflix-style genre browsing, the resume/Keep-Watching experience, search (grouped, live, cross-type, scoped), and the provider "Check Status" diagnostics.

The defects below are mostly **polish and feedback gaps** rather than broken core flows. The two that most affect daily use are the **non-functional subtitle selection** and the **silent catalog refresh**.

### Prioritized issues (engineer triage)

| # | Sev | Area | Issue |
|---|-----|------|-------|
| 1 | 🐞 High | Player | Subtitle selection appears non-functional — can't turn Off or switch track (stays on default); subtitles also ON by default (§7) |
| 2 | 🐞 High | Live TV | ~10 channels render with **blank names** in All Channels (§2) |
| 3 | 🐞 Med | Series | Series name & SxxEyy **duplicated** in player title and Keep Watching label ("Black Mirror — Black Mirror - S02E01 - …") (§4) |
| 4 | 🐞 Med | Settings | Catalog **refresh has no feedback** (no progress/toast) and "Last refreshed" timestamp doesn't update, though data does refresh (§6) |
| 5 | 🐞 Med | Settings | Appearance **Density toggle** has no visible effect and doesn't persist (reverts to Comfortable) (§6) |
| 6 | 🐞 Med | Lists | **Delete list has no confirmation/undo** — instant data loss (§1); verify provider Delete too (§6) |
| 7 | ⚠️ Med | Global | **No keyboard shortcuts** anywhere — Esc doesn't close player/modals/search; space doesn't pause (§2, §5, §7) |
| 8 | ⚠️ Low | Movies | Movie **detail page doesn't show in-progress state** (still "Play", no progress bar) though Home thumbnail does (§3) |
| 9 | ⚠️ Low | Home | Carousels lack scroll arrows; genre rows have tiny ones — inconsistent affordance (§1, §3) |
| 10 | ⚠️ Low | Lists | Add-to-list gives no toast; empty-list cover shows "?" placeholder grid (§1) |

---

## 1. Home screen

**What works**
- ✅ Clean, centered top nav (provider pill · Home · Live TV · Movies · Series · Settings · search · refresh). Reads clearly and is consistent across screens.
- ✅ Landing page is genuinely useful: "My Lists" + "Popular Movies (30)" + "Popular Series (30)" carousels populated with real provider artwork.
- ✅ Adding a title to a list is reactive — "To Watch" count updated 4 → 5 immediately, and the list-detail grid reflected the new item.
- ✅ Navigation state is preserved: after opening a movie detail and pressing Back, the Popular Movies carousel kept its horizontal scroll position.
- ✅ Series cards show release year under the title; movie cards show year in the title — helpful context.

**Issues / friction**
- ⚠️ Carousels have **no on-screen left/right scroll arrows**. Horizontal movement relies on a mouse wheel that supports horizontal scroll, trackpad, or dragging the thin scrollbar. Mouse-only users on a vertical-wheel mouse will struggle to scroll the rows. 💡 Add hover-reveal chevron buttons at the row edges.
- ⚠️ Adding to a list gives **no toast / confirmation** — only the small checkbox turns green inside the still-open dropdown. Easy to miss. 💡 Consider a brief toast ("Added to To Watch") and/or persist the "in list" badge on the card.
- ⚠️ **No way to remove an item from a list within the list-detail view.** Hovering a poster only scales it; there is no remove (×) affordance. User must open the title's detail page and untick the list. 💡 Add a hover remove control or a right-click context menu in list view.
- 💡 The list-detail view exposes a **"Delete"** button for the list. Verify the default "To Watch" list cannot be left in a confusing state if deleted (does it regenerate? is there an undo?). Not destructively tested.
- 💡 Green "✓" badge appears on some Popular Movies cards (e.g. Apex) — presumably "watched" or "in a list". Meaning is not labeled; a tooltip/legend would help.

**New list / delete flow**
- ✅ "New list" opens a clean modal (name field + Cancel/Create); created list appears immediately with count updating (1 → 2).
- ✅ Empty list-detail has a good empty state: _"This list is empty — Add movies, series, or channels with the 'Add to list…' option from any title's right-click menu."_ (Confirms a right-click context menu exists — good for power users.)
- 🐞/⚠️ **Deleting a list has no confirmation and no undo.** Clicking "Delete" removed the list instantly. For a populated list this is real data loss. 💡 Add a confirm dialog ("Delete 'X' and its N items?") or an undo toast.
- ⚠️ Empty-list **cover thumbnail on Home shows a 2×2 grid of "?" placeholders**, which looks broken/unfinished. 💡 Use a neutral empty-list icon instead.
- 💡 New-list modal: pressing Enter to submit / Esc to cancel not verified — ensure keyboard submission works (only tested via Create button).

---

## 2. Live TV browser

**What works**
- ✅ Two-pane layout (category sidebar + channel list) is clear and familiar.
- ✅ Category selection is instant; channel list virtualizes a large catalog smoothly.
- ✅ "Filter channels…" box filters live and shows a clear (×) button; results update per keystroke.
- ✅ Sidebar sort toggle switches between **A-Z** and **PROVIDER** (original provider order) — useful, and provider order surfaces the catalog's intended grouping (USA ENTERTAINMENT, USA NEWS, …).
- ✅ **Channel playback is excellent**: clicking ESPN HD started a real live stream full-screen with fast time-to-first-frame.
- ✅ Closing the player (× control) returns to the channel list and **preserves the selected category** and scroll position.
- ✅ When filtered to a single category, the redundant category tag on each row is hidden (contextual).

**Issues / friction**
- 🐞 **Blank channel names.** In "All Channels", the first ~10 rows (WNBA logo + "WNBA" tag) render with **no channel name text at all** — just an empty row. Either the names are null in the data or the UI is dropping them. Result: unidentifiable, near-invisible rows. 💡 Fall back to a placeholder (stream id / "Untitled channel") and/or filter out empty entries.
- ⚠️ The filter box is **scoped to the currently-selected category**, not global. Reasonable, but a user in a category who wants to search everything must first click "All Channels". 💡 Consider a hint, or a toggle to search all.
- ⚠️ **Player control bar auto-hides very quickly and is a thin strip at the extreme bottom edge.** Easy to miss and fiddly to hit (volume, audio-track, aspect, fullscreen, ×). 💡 Increase the hover grace period and the bar's hit area.
- ⚠️ **Spacebar does not pause** playback (live stream kept advancing). Standard media shortcuts (space = play/pause, f = fullscreen, m = mute, Esc = close) should be wired. 💡 Verify/define player keyboard shortcuts.
- ⚠️ The only way to exit the player is the small **×** in the auto-hiding bar; there's no always-visible Back affordance, and Esc-to-close was not confirmed. 💡 Support Esc and/or a persistent back control.

**Minor / ideas**
- 💡 No "now playing"/last-watched indicator on the channel you just viewed, and no "Recently watched" channels row.
- 💡 The LIVE badge alongside a running timer (0:36 → 1:29) is ambiguous about what the timer represents (session elapsed vs. buffer/time-shift position). Clarify, or hide the timer for pure-live streams.

---

## 3. Movies (VOD)

**What works**
- ✅ "All Movies" uses a polished **Netflix-style genre-row layout** (POPULAR, 4K, Action, Adventure…) with a genre sidebar. Looks great and is easy to browse.
- ✅ Selecting a genre switches to a **dense alphabetical grid** with smooth vertical scroll; posters load quickly across a large catalog.
- ✅ Detail page is clean: relevant backdrop (Alien), poster, runtime (1h 57m), rating (★ 8.1 — /10 scale), genre chips, full synopsis, and Play / Open in External Player / Add to list.
- ✅ **VOD playback works well**: correct letterboxing for aspect ratio, fast start.
- ✅ **Rich VOD seek bar**: full timeline with current/total time (e.g. 1:01:10 / 2:02:06); clicking the bar seeks **accurately**.
- ✅ Subtitles render during playback.
- ✅ **Watch progress + resume is solid**: after exiting at ~50%, clicking Play shows a "Resume playback?" modal ("Resume from 1:01:42" / "Start from beginning"); Resume correctly jumps to the saved position.
- ✅ **"Keep Watching" row** appears at the top of Home once something is in progress, with a **progress bar overlaid on the thumbnail**.

**Issues / friction**
- ⚠️ **The detail page does not reflect in-progress state.** After watching 50%, the page still shows a generic "Play" button with no progress bar — the resume only surfaces as a modal *after* you click Play. 💡 On the detail page, change "Play" → "Resume from 1:01:42" with a secondary "Start over", and show a progress bar on/under the poster. Keeps detail consistent with the Home thumbnail.
- ⚠️ Player keyboard shortcuts still missing here too: **Esc did not close** the VOD player; space pause unverified/likely unbound (see Live TV).
- ⚠️ Genre-row **scroll chevrons are very small/subtle** at the row edges, and Home's carousels have none at all — inconsistent affordance across screens. 💡 Standardize prominent hover chevrons on all horizontal rows.

**Not yet tested / to verify**
- "Open in External Player" (launches a system player — not exercised to avoid spawning external apps).
- Subtitle/audio-track selection controls (icons present in the player bar).
- Subtitle/audio-track selection controls — confirmed icons present; selection UX not exercised.

**Resolved during testing**
- ✅ Clicking a "Keep Watching" tile goes **straight to the Resume modal** (no detail detour) — good.
- ⚠️ The "Resume playback?" modal has **no explicit Cancel/close button** (only Resume / Start from beginning). Clicking the backdrop dismisses it, but that's not discoverable. 💡 Add a Cancel button and Esc-to-dismiss.

---

## 4. Series

**What works**
- ✅ Layout is consistent with Movies (genre sidebar + genre rows; "All Series" landing). Predictable and easy.
- ✅ Series detail is well organized: relevant backdrop, poster, year, genre chips, synopsis, Add to list, and an **"Episodes"** block.
- ✅ **Season dropdown** (Black Mirror showed Seasons 1–7) switches the episode list correctly and instantly.
- ✅ Each episode row shows a **thumbnail, title, "Episode N · runtime", and description** — rich and scannable.
- ✅ Hovering an episode reveals a **play overlay** and a **"⋯" context menu**.
- ✅ **Episode playback works** with a current/total seek bar (0:08 / 48:39).
- ✅ **Episode progress bar is shown on the episode thumbnail** in the detail view (note: better than the movie detail page, which shows no progress — see §3).
- ✅ In-progress episodes appear in Home **"Keep Watching"**, ordered by recency (Black Mirror jumped above Alien after I watched it).

**Issues / friction**
- 🐞 **Duplicated series name / episode code in titles.** The player title bar reads **"Black Mirror — Black Mirror - S02E01 - Be Right Back"**, and the Keep Watching label reads **"S2:E1 · Black Mirror - S02E01 - B…"**. The app composes `{series} — {episodeTitle}` / `S2:E1 · {episodeTitle}` but the provider's episode title already contains the series name and `S02E01`. Result: "Black Mirror" appears twice and the episode is numbered twice in two formats. 💡 Strip the redundant series/SxxEyy prefix from the provider episode title, or compose from structured fields only → e.g. **"Black Mirror · S2:E1 — Be Right Back"**.
- ⚠️ The series detail has **no top-level "Play / Continue" CTA** — to start or resume you must scroll to the Episodes list and pick. 💡 Add a "Resume S2:E1" / "Play S1:E1" button near the title, mirroring movies.

**Not yet tested**
- Episode "⋯" context menu contents (likely Add to list / mark watched).
- Auto-advance to the next episode at end of playback.
- Per-episode resume prompt (movie-style modal) when replaying a partially-watched episode.

---

## 5. Search

**What works**
- ✅ Search opens as a fast centered overlay ("Search channels, movies, and series…").
- ✅ Results are **grouped by type** (LIVE TV / MOVIES / SERIES) with a per-group count and a **"Show all N results"** link when truncated.
- ✅ **Live, per-keystroke** results; "avatar" returned Movies (7) + Series (2), "espn" returned Live TV (100) + Movies (11).
- ✅ **Cross-type**: live channels show logo + name + category tag; movies/series show posters with year.
- ✅ **Scope chips** (All / Live TV / Movies / Series) filter correctly; scoping to Movies/Series reveals an extra **"All genres"** sub-filter — nice power feature.
- ✅ Clicking a result navigates straight to the correct detail page.
- ✅ Clean **no-results state**: _"No results for 'zzqxqzwq'. Check the spelling or try a broader term."_

**Issues / friction**
- ⚠️ **Esc does not close the search overlay** (had to click outside). Combined with the player and resume modal, this is an **app-wide gap: the Escape key is not wired anywhere**. 💡 Standardize Esc = close/back for all overlays, modals, and the player.
- 💡 The Live TV group caps at "100 results" — confirm this is a deliberate cap and that "Show all" reveals the rest.
- 💡 No keyboard navigation of results (arrow keys + Enter) was apparent — would speed up keyboard-first use.

---

## 6. Settings & catalog refresh

**What works**
- ✅ Clean Settings with sub-nav: Providers / Playback / Appearance.
- ✅ Provider card shows type badge (XTREAM), URL, and "Last refreshed" time, with Check Status / Edit / Delete + Add Provider.
- ✅ **"Check Status" is excellent**: green panel with "Connected successfully", Status: Active, **subscription expiry date**, and **"Connections: 0 / 3 active"** — genuinely useful account diagnostics.
- ✅ **Edit/Add forms** are well designed: Xtream/M3U type toggle, URL, username, password, with Test Connection. Helpful placeholders ("http://example.com:8080").
- ✅ **Good password security**: the Edit form's password field shows "Leave blank to keep current" and never displays the stored secret (consistent with keychain-only storage).
- ✅ **Form validation**: Test Connection with empty fields shows an inline red error ("Provider name is required.").
- ✅ Playback settings: default external player dropdown (mpv) + Hardware-decode toggle with a clear "takes effect on next stream" note.
- ✅ Manual catalog **refresh actually works**: triggering ↻ updated the Live TV catalog from yesterday's (6/23) WNBA schedule to today's (6/24) games.

**Issues / friction**
- 🐞 **Appearance → Density toggle appears non-functional / non-persistent.** Switching to "Compact" produced **no visible change** on the Movies grid or Live TV list, and after navigating away and back the setting had **reverted to "Comfortable"**. Either it isn't wired to layout, or it isn't persisted (and the control doesn't reflect the stored value). Needs investigation.
- 🐞 **Manual refresh gives zero feedback.** Clicking ↻ shows **no spinner, no progress banner, and no toast**, and the provider's **"Last refreshed" timestamp did not update** (still 10:52:51 AM) even though the catalog data demonstrably changed. The user cannot tell a refresh started, is running, or finished. 💡 Add a "Refreshing… (N/total)" indicator (the backend already emits `catalog:refresh_progress`), and update "Last refreshed" on completion.
- ⚠️ Theme is **Dark-only** ("Light theme is planned for a future release") — fine, but the disabled "Dark" button looks like it might be clickable; consider styling it clearly as the only/active option.

**Not tested (intentionally, to avoid altering your setup)**
- "Delete" provider (would remove the only configured provider). 💡 Verify it shows a confirmation dialog — the list-Delete in §1 did **not**, so provider-Delete should be checked for the same gap.
- "Test Connection" / "Add Provider" with real new credentials.

---

## 7. Player (cross-cutting: Live TV, Movies, Series)

**What works**
- ✅ Fast, reliable playback across all three content types using real streams.
- ✅ Correct aspect-ratio letterboxing; smooth decode (hardware decode on).
- ✅ VOD/episode **seek bar is accurate** (current/total time, click-to-seek).
- ✅ **Audio-track menu works** (showed "eng · ac3 ✓").
- ✅ **Fullscreen toggle works** — hides the OS title bar and fills the screen; toggles back cleanly.
- ✅ **Close (×)** reliably returns to the previous screen.
- ✅ Resume + Keep Watching integration (see §3/§4).

**Issues / friction**
- 🐞 **Subtitle selection appears non-functional.** Selecting "Off" (twice) and switching to "eng · subrip" all failed — the active track stayed on "eng · dvd_subtitle ✓" across multiple attempts, and on-screen subtitles did not change. Users currently **cannot turn subtitles off or change track**. (High-confidence but please verify; the menu renders correctly, only the selection has no effect.)
- ⚠️ **Subtitles are ON by default** (a subtitle track is auto-selected). Many users expect subs Off unless enabled — combined with the bug above, they can't disable them. 💡 Default to Off, and fix selection.
- ⚠️ **No keyboard shortcuts.** Space does not pause; Esc does not close; f / m / arrows not wired. This is consistent across the player, modals, and search overlay — an app-wide accessibility gap. 💡 Implement the standard set (space, f, m, ←/→ seek, ↑/↓ volume, Esc).
- ⚠️ Control bar is a **thin, fast-auto-hiding strip at the extreme bottom**; targets (volume, track menus, fullscreen, ×) are small and easy to miss. 💡 Larger hit areas + longer hover grace.
- 💡 Track-menu labels expose codec names ("eng · dvd_subtitle", "eng · subrip", "eng · ac3"). Friendlier labels ("English", "English (SRT)", "English 5.1") would read better; de-duplicate identical entries.

---

## 8. Cross-cutting / global observations

- **Consistency wins.** The top nav, detail-page template (backdrop / poster / metadata / chips / actions / synopsis), genre sidebar + rows, and the player bar are reused coherently across Live TV, Movies, and Series. This makes the app feel polished and learnable.
- **Keyboard support is the biggest systemic gap.** Esc closes nothing (player, resume modal, search overlay); space doesn't pause. A single keyboard-handling pass would lift the whole app's usability and accessibility.
- **Feedback/confirmation gaps recur.** Several actions complete silently or destructively: add-to-list (no toast), list delete (no confirm/undo), catalog refresh (no progress, stale timestamp). A shared toast/confirm pattern would address §1, §6 at once.
- **Title/label composition needs de-duplication.** The series-name/episode-code duplication (§4) suggests labels are built by string concatenation over provider fields that already embed those values. A normalize step (strip known series prefix + SxxEyy) would fix it everywhere it surfaces (player title, Keep Watching).
- **Empty/placeholder handling.** Blank channel names (§2) and the "?" empty-list cover (§1) are the two spots where missing data leaks into the UI; both want graceful fallbacks.
- **Settings ↔ UI wiring.** Density (§6) is the one setting that didn't visibly do anything or persist — worth confirming the Appearance store is actually read by the layout and saved to disk.

### Suggested next steps for engineering
1. Fix subtitle track selection + default Off (§7) — highest user impact.
2. Add a graceful fallback for blank channel names (§2).
3. Normalize episode/series title composition (§4).
4. Wire catalog-refresh progress + timestamp update (already emitted by backend) (§6).
5. Add an app-wide keyboard handler (Esc/space/f/m/arrows) (§2,§5,§7).
6. Add confirm/undo for destructive list & provider deletes (§1,§6).
7. Audit the Appearance/Density setting end-to-end (§6).

---

### Test coverage notes
- All findings from the real local release build (`src-tauri/target/release/proscenium.exe`) against the existing **SRP Tech App** Xtream provider with live data. No mock/fake data used.
- Playback verified on real streams: ESPN HD (live), Alien 1979 (VOD movie), Black Mirror S2E1 (series episode).
- Intentionally **not** exercised to avoid side effects: deleting the provider, "Open in External Player" (would spawn an external app), adding a new provider with real credentials. A temporary "QA Test List" was created and deleted during testing; one item (Chum 2026) was added to your "To Watch" list (5 items) and a couple of titles now appear in "Keep Watching" as a natural result of playback testing.
