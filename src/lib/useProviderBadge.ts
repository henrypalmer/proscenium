import { useCatalogStore } from "../store/catalogStore";

/**
 * The provider name to badge on a catalog card, or `null` when only one provider
 * is enabled (no need to disambiguate). Milestone 39: merged catalogs show which
 * provider each item came from.
 */
export function useProviderBadge(providerId: string): string | null {
  return useCatalogStore((s) =>
    s.enabledProviders.length > 1
      ? (s.enabledProviders.find((p) => p.id === providerId)?.name ?? null)
      : null,
  );
}
