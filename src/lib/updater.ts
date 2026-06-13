import { inTauri } from "./tauri";

/**
 * Auto-updater launch check (spec §13 / Milestone 7). Queries the configured
 * update endpoint once on startup. If an update is available it is downloaded
 * and installed, then the app relaunches. Any failure (offline, no endpoint,
 * no update) is swallowed — a failed update check must never block launch.
 *
 * No-ops outside the Tauri shell so the browser dev build is unaffected.
 */
export async function checkForUpdatesOnLaunch(): Promise<void> {
  if (!inTauri) return;
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) {
      console.info("[proscenium] update check: already on the latest version");
      return;
    }
    console.info(
      `[proscenium] update available: ${update.version} — downloading…`,
    );
    await update.downloadAndInstall();
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  } catch (e) {
    console.info("[proscenium] update check skipped:", e);
  }
}
