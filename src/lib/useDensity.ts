import { useSettingsStore } from "../store/settingsStore";
import type { UiDensity } from "../types";

/**
 * The active content density (spec §9 Typography & Density, Milestone 24).
 * Drives card/grid/row sizing on the browse surfaces; defaults to "comfortable"
 * until settings load. Reactive — components re-render when the user switches it
 * in Settings → Appearance.
 */
export function useDensity(): UiDensity {
  const value = useSettingsStore((s) => s.settings?.uiDensity);
  return value === "compact" ? "compact" : "comfortable";
}
